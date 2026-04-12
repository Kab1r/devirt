//! Transparent devirtualization for Rust trait objects, with `#![no_std]` support.
//!
//! Hot types get **vtable-pointer-comparison dispatch**: the dispatch shim
//! extracts the vtable pointer from the `&dyn Trait` fat pointer and compares
//! it against the compile-time-known vtable address for each hot type. On
//! match, the data pointer is reinterpreted as `&HotType` and the method is
//! called directly (fully inlined). On miss, the shim falls through
//! to a single vtable call via the hidden `__spec_*` method.
//!
//! This eliminates the indirect call entirely on hot paths — there is no
//! vtable call on the hot-dispatch branch, only a `cmp + je` against a
//! RIP-relative vtable address. Callers use plain `dyn Trait` — no
//! wrappers, no special calls.
//!
//! # Architecture
//!
//! The trait definition (`#[devirt::devirt(Hot1, Hot2)]` or
//! `devirt::devirt! { trait Foo [Hot1, Hot2] { ... } }`) generates:
//! - A hidden inner trait `__XImpl` with `__spec_*` method declarations
//! - Two `#[doc(hidden)]` inherent helpers on `dyn X`:
//!   `__devirt_raw_parts` (extracts `[data, vtable]` from a fat pointer)
//!   and `__devirt_vtable_for::<T>()` (returns the compiler-assigned
//!   vtable address for `(T, X)`)
//! - Inherent dispatch methods on `dyn X` whose bodies compare the
//!   runtime vtable pointer against each hot type's vtable and dispatch
//!   directly on match, or fall through to `__spec_*` otherwise
//! - A blanket impl: `impl<T: __XImpl> X for T {}`
//!
//! The impl (`#[devirt::devirt]` or `devirt::devirt! { impl Foo for T { ... } }`)
//! generates:
//! - `impl __XImpl for ConcreteType { ... }` with the `__spec_*` bodies
//!
//! Both the proc-macro attribute and the declarative macro delegate to
//! `__devirt_define!`, a `#[doc(hidden)]` internal macro that contains
//! all dispatch expansion logic.
//!
//! # Usage
//!
//! ```ignore
//! // With proc-macro attribute (default):
//! #[devirt::devirt(HotType1, HotType2)]
//! pub trait MyTrait {
//!     fn method(&self) -> ReturnType;
//! }
//!
//! #[devirt::devirt]
//! impl MyTrait for HotType1 {
//!     fn method(&self) -> ReturnType { ... }
//! }
//!
//! // With declarative macro (default-features = false):
//! devirt::devirt! {
//!     pub MyTrait [HotType1, HotType2] {
//!         fn method(&self) -> ReturnType;
//!     }
//! }
//!
//! devirt::devirt! {
//!     impl MyTrait for HotType1 {
//!         fn method(&self) -> ReturnType { ... }
//!     }
//! }
//! ```
//!
//! # LTO
//!
//! LTO is **not required**. All dispatch logic expands via macros into
//! the user's crate, so there are no cross-crate function calls to
//! inline. Vtable deduplication works within a single crate via COMDAT
//! groups, even with multiple codegen units.
//!
//! # Performance characteristics
//!
//! Hot-type dispatch is a single `cmp + je` against a RIP-relative vtable
//! address followed by an inlined method body — **no indirect call**. Cold
//! types pay a small branch-per-hot-type penalty on the dispatch shim
//! before the vtable fallback.
//!
//! The crate is most effective when hot types dominate the population (80%+
//! of trait objects). It is especially effective on *shuffled* collections
//! where the CPU's indirect branch predictor cannot learn the call pattern.
//!
//! # Safety
//!
//! The dispatch shim uses three small `unsafe` operations:
//!
//! 1. `transmute::<*const dyn Trait, [usize; 2]>` to split a fat pointer
//!    into `(data, vtable)` halves. Safe because `*const dyn Trait` is a
//!    two-usize fat pointer — verified by a compile-time `size_of` assertion
//!    and a `#[cfg(test)]` ordering check.
//! 2. Reinterpreting the data half as `*const HotType` / `*mut HotType` on
//!    a vtable match. Safe because distinct types have distinct vtables
//!    (vtables encode size, alignment, and drop glue), and the compiler
//!    deduplicates vtables for the same `(Type, Trait)` pair under LTO.
//! 3. Coercing a dangling `*const T` to `*const dyn Trait` inside
//!    `__devirt_vtable_for` to read the vtable address. Safe because
//!    coercion is a metadata-attaching operation that does not dereference
//!    the data pointer.
//!
//! # Limitations
//!
//! **Object safety required.** All trait methods must be object-safe — no
//! generic parameters, no `Self` in return position. The generated blanket
//! impl requires `dyn Trait` to work, so violating object safety will produce
//! errors pointing at the generated impl rather than the method definition.
//!
//! **Hot types must be `'static`.** The vtable-probe helper coerces a
//! `*const T` to `*const dyn Trait`, which inherits `dyn Trait`'s default
//! `'static` bound. Types with borrowed fields cannot currently be listed
//! as hot types.

#![no_std]
// SAFETY: the crate's entire purpose is safely-encapsulated unsafe dispatch.
// Individual unsafe sites are documented with `SAFETY` comments and verified
// by Miri, Kani, and Verus.
#![allow(unsafe_code)]

#[cfg(kani)]
extern crate kani;

#[doc(hidden)]
pub use paste::paste as __paste;

#[doc(hidden)]
#[macro_export]
macro_rules! __devirt_define {
    (@trait
        $(#[$meta:meta])*
        $vis:vis $trait_name:ident [$($hot:ty),+ $(,)?] {
            $($methods:tt)*
        }
    ) => {
        $crate::__paste! {
            #[doc(hidden)]
            $vis trait [<__ $trait_name Impl>] {
                $crate::__devirt_define!{@spec_decl $($methods)*}
            }

            // Compile-time sanity check: `*const dyn Trait` must be a fat
            // pointer of exactly two `usize`s. If a future Rust edition
            // changes this, compilation fails loudly rather than producing
            // UB at runtime.
            const _: () = assert!(
                ::core::mem::size_of::<*const dyn $trait_name>()
                    == 2 * ::core::mem::size_of::<usize>()
            );

            // Inherent helpers on `dyn $trait_name` that expose the fat
            // pointer's `(data, vtable)` halves and the compiler-assigned
            // vtable address for a concrete hot type. These are `#[inline(
            // always)]` so LTO folds them into the dispatch shim.
            impl<'__devirt> dyn $trait_name + '__devirt {
                /// Split a fat pointer into `[data, vtable]`.
                #[doc(hidden)]
                #[inline(always)]
                pub fn __devirt_raw_parts(this: &Self) -> [usize; 2] {
                    // SAFETY: `&(dyn $trait_name + '_)` is a two-`usize`
                    // fat pointer (verified by the compile-time
                    // `size_of` assertion above) laid out as
                    // `[data, vtable]`. Transmuting to `[usize; 2]`
                    // only reinterprets bits — the data half is still
                    // borrowed for the duration of `this`, so the
                    // result may not outlive the borrow.
                    unsafe { ::core::mem::transmute::<
                        &Self, [usize; 2],
                    >(this) }
                }

                /// Vtable pointer for the `(T, Self)` pair.
                #[doc(hidden)]
                #[inline(always)]
                pub fn __devirt_vtable_for<
                    T: [<__ $trait_name Impl>] + 'static,
                >() -> usize {
                    // A dangling, non-null, aligned `*const T`. We never
                    // dereference it — the coercion below only reads the
                    // vtable metadata the compiler attaches.
                    let fake: *const T = ::core::ptr::without_provenance(
                        ::core::mem::align_of::<T>(),
                    );
                    // Coercion is a metadata-attaching op; the resulting
                    // fat pointer's vtable half is the `(T, $trait_name)`
                    // vtable selected by the compiler. Its data half is
                    // `fake`, which we discard.
                    let fat: *const Self = fake;
                    // SAFETY: `*const Self` (dyn trait fat pointer) is
                    // two `usize`s by the compile-time assertion above.
                    // We read only the vtable half; the dangling data
                    // half is discarded without dereferencing.
                    let __parts: [usize; 2] = unsafe {
                        ::core::mem::transmute::<
                            *const Self, [usize; 2],
                        >(fat)
                    };
                    __parts[1]
                }
            }

            // Inherent dispatch methods on `dyn $trait_name`. These
            // contain the vtable-comparison hot-path and take priority
            // over the trait's default methods during method resolution,
            // so a call like `dyn_trait.method()` reaches this block
            // before falling back to the trait method. Putting the cast
            // `self as *const dyn $trait_name` here (where `Self = dyn
            // $trait_name`) avoids the `Self: Sized` requirement that
            // would otherwise arise in a default method body.
            impl<'__devirt> dyn $trait_name + '__devirt {
                $crate::__devirt_define!{
                    @inherent_decl
                    [<__ $trait_name Impl>],
                    $trait_name,
                    [$($hot),+],
                    $($methods)*
                }
            }

            // The public trait is a thin marker over the hidden inner
            // trait: it carries no methods of its own, so `dyn
            // $trait_name` has no trait methods to conflict with the
            // inherent dispatch methods emitted above. Methods named
            // from the user's declaration resolve unambiguously to the
            // inherent block.
            //
            // Concrete-type callers that want to bypass dispatch
            // entirely can either call `<$trait_name>::$method` via
            // an explicit dyn coercion `(&t as &dyn $trait_name).$method
            // (...)` or call `<T as __${trait_name}Impl>::__spec_$method
            // (&t, ...)` via UFCS.
            $(#[$meta])*
            $vis trait $trait_name: [<__ $trait_name Impl>] {}

            impl<T: [<__ $trait_name Impl>] + ?Sized> $trait_name for T {}
        }
    };

    // ── @spec_decl ──────────────────────────────────────────────────────────

    (@spec_decl
        $(#[$_attr:meta])*
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            fn [<__spec_ $method>](&self $(, $arg: $argty)*) $(-> $ret)?;
        }
        $crate::__devirt_define!{@spec_decl $($rest)*}
    };

    (@spec_decl
        $(#[$_attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            fn [<__spec_ $method>](&mut self $(, $arg: $argty)*) $(-> $ret)?;
        }
        $crate::__devirt_define!{@spec_decl $($rest)*}
    };

    (@spec_decl) => {};

    // ── @inherent_decl ──────────────────────────────────────────────────────
    //
    // Emits inherent methods on `impl dyn $trait_name { ... }` that do
    // the vtable-comparison dispatch. Because `Self = dyn $trait_name`
    // in this context, the cast `self as *const dyn $trait_name` is a
    // simple reference-to-pointer cast with no `Sized` requirement.

    // &self, non-void
    (@inherent_decl $inner:ident, $trait_name:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*) -> $ret:ty;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            #[doc(hidden)]
            pub fn $method(&self $(, $arg: $argty)*) -> $ret {
                $crate::__devirt_define!(
                    @dispatch_ref
                    $inner, $trait_name, self, $method, ($($arg),*), [$($hot),+]
                )
            }
        }
        $crate::__devirt_define!{@inherent_decl $inner, $trait_name, [$($hot),+], $($rest)*}
    };

    // &self, void
    (@inherent_decl $inner:ident, $trait_name:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*);
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            #[doc(hidden)]
            pub fn $method(&self $(, $arg: $argty)*) {
                $crate::__devirt_define!(
                    @dispatch_void
                    $inner, $trait_name, self, $method, ($($arg),*), [$($hot),+]
                )
            }
        }
        $crate::__devirt_define!{@inherent_decl $inner, $trait_name, [$($hot),+], $($rest)*}
    };

    // &mut self, non-void
    (@inherent_decl $inner:ident, $trait_name:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*) -> $ret:ty;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            #[doc(hidden)]
            pub fn $method(&mut self $(, $arg: $argty)*) -> $ret {
                $crate::__devirt_define!(
                    @dispatch_mut
                    $inner, $trait_name, self, $method, ($($arg),*), [$($hot),+]
                )
            }
        }
        $crate::__devirt_define!{@inherent_decl $inner, $trait_name, [$($hot),+], $($rest)*}
    };

    // &mut self, void
    (@inherent_decl $inner:ident, $trait_name:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*);
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            #[doc(hidden)]
            pub fn $method(&mut self $(, $arg: $argty)*) {
                $crate::__devirt_define!(
                    @dispatch_mut_void
                    $inner, $trait_name, self, $method, ($($arg),*), [$($hot),+]
                )
            }
        }
        $crate::__devirt_define!{@inherent_decl $inner, $trait_name, [$($hot),+], $($rest)*}
    };

    (@inherent_decl $inner:ident, $trait_name:ident, [$($hot:ty),+],) => {};

    // ── @dispatch_ref: &self, non-void ─────────────────────────────────────
    //
    // Expands to: split the fat pointer into `[data, vtable]`, then for
    // each hot type compare the extracted vtable against the
    // compile-time-known vtable for `(HotType, Trait)`. On match,
    // reinterpret the data pointer as `&HotType` and tail-call the hot
    // type's `__spec_*` method directly (fully inlined under LTO). On
    // full miss, fall through to a single `__spec_*` call via the inner
    // trait bound, which compiles to the usual vtable indirect call.
    //
    // Hot types are consumed by recursive expansion (rather than `$()+`
    // repetition) so the macro parser doesn't have to reconcile the
    // mismatched repetition depths of `$hot` and `$arg`.

    (@dispatch_ref $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$($hot:ty),+]
    ) => {{
        let __raw: [usize; 2] =
            <dyn $trait_name>::__devirt_raw_parts($this);
        $crate::__devirt_define!(@dispatch_ref_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($hot),+], __raw)
    }};

    (@dispatch_ref_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$first:ty $(, $rest:ty)*], $raw:ident
    ) => {{
        if $raw[1] == <dyn $trait_name>::__devirt_vtable_for::<$first>() {
            // Bind the data-half to a local `*const $first` outside
            // any `unsafe` block so that no metavariable is expanded
            // inside `unsafe { ... }` — clippy's
            // `macro_metavars_in_unsafe` lint flags the latter.
            let __p: *const $first = $raw[0] as *const $first;
            // SAFETY: vtable identity implies type identity. The data
            // half is the original `&$first` the caller coerced into
            // the fat pointer, valid for at least the lifetime of
            // `$this`'s borrow.
            let __concrete: &$first = unsafe { &*__p };
            return $crate::__paste! {
                __concrete.[<__spec_ $method>]($($arg),*)
            };
        }
        $crate::__devirt_define!(@dispatch_ref_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($rest),*], $raw)
    }};

    (@dispatch_ref_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [], $raw:ident
    ) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }($this $(, $arg)*)
    };

    // ── @dispatch_void: &self, void ─────────────────────────────────────────

    (@dispatch_void $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$($hot:ty),+]
    ) => {{
        let __raw: [usize; 2] =
            <dyn $trait_name>::__devirt_raw_parts($this);
        $crate::__devirt_define!(@dispatch_void_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($hot),+], __raw)
    }};

    (@dispatch_void_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$first:ty $(, $rest:ty)*], $raw:ident
    ) => {{
        if $raw[1] == <dyn $trait_name>::__devirt_vtable_for::<$first>() {
            let __p: *const $first = $raw[0] as *const $first;
            // SAFETY: see @dispatch_ref_chain above.
            let __concrete: &$first = unsafe { &*__p };
            $crate::__paste! {
                __concrete.[<__spec_ $method>]($($arg),*);
            }
            return;
        }
        $crate::__devirt_define!(@dispatch_void_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($rest),*], $raw)
    }};

    (@dispatch_void_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [], $raw:ident
    ) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }($this $(, $arg)*)
    };

    // ── @dispatch_mut: &mut self, non-void ─────────────────────────────────
    //
    // The `&mut` arms go through a raw `*mut` dereference rather than
    // constructing a named `&mut $hot` binding. This keeps the hot-branch
    // reborrow scoped to the single method call expression and avoids
    // aliasing the still-live `&mut dyn $trait_name` under Stacked
    // Borrows. The cold fallback path uses `self` directly.

    (@dispatch_mut $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$($hot:ty),+]
    ) => {{
        // Shared reborrow of `&mut dyn $trait_name` to read the fat
        // pointer halves. The reborrow is scoped to the call to
        // `__devirt_raw_parts` and released before we construct any
        // `&mut $first` on the hot path, so there is no overlapping
        // mutable alias.
        let __raw: [usize; 2] =
            <dyn $trait_name>::__devirt_raw_parts(&*$this);
        $crate::__devirt_define!(@dispatch_mut_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($hot),+], __raw)
    }};

    (@dispatch_mut_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$first:ty $(, $rest:ty)*], $raw:ident
    ) => {{
        if $raw[1] == <dyn $trait_name>::__devirt_vtable_for::<$first>() {
            let __p: *mut $first = $raw[0] as *mut $first;
            // SAFETY: vtable match → the underlying storage is a
            // `$first`. The reborrow to `&mut $first` is scoped to
            // this branch (which returns immediately), so the
            // enclosing `&mut dyn $trait_name` is not accessed again
            // while the reborrow is live.
            let __ref: &mut $first = unsafe { &mut *__p };
            return $crate::__paste! {
                __ref.[<__spec_ $method>]($($arg),*)
            };
        }
        $crate::__devirt_define!(@dispatch_mut_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($rest),*], $raw)
    }};

    (@dispatch_mut_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [], $raw:ident
    ) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }(&mut *$this $(, $arg)*)
    };

    // ── @dispatch_mut_void: &mut self, void ────────────────────────────────

    (@dispatch_mut_void $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$($hot:ty),+]
    ) => {{
        let __raw: [usize; 2] =
            <dyn $trait_name>::__devirt_raw_parts(&*$this);
        $crate::__devirt_define!(@dispatch_mut_void_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($hot),+], __raw)
    }};

    (@dispatch_mut_void_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [$first:ty $(, $rest:ty)*], $raw:ident
    ) => {{
        if $raw[1] == <dyn $trait_name>::__devirt_vtable_for::<$first>() {
            let __p: *mut $first = $raw[0] as *mut $first;
            // SAFETY: see @dispatch_mut_chain above.
            let __ref: &mut $first = unsafe { &mut *__p };
            $crate::__paste! {
                __ref.[<__spec_ $method>]($($arg),*);
            }
            return;
        }
        $crate::__devirt_define!(@dispatch_mut_void_chain
            $inner, $trait_name, $this, $method, ($($arg),*), [$($rest),*], $raw)
    }};

    (@dispatch_mut_void_chain $inner:ident, $trait_name:ident, $this:tt, $method:ident,
        ($($arg:expr),*), [], $raw:ident
    ) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }(&mut *$this $(, $arg)*)
    };

    // ── @impl: implement a devirtualized trait for a concrete type ──────────

    (@impl $trait_name:ident for $type:ty {
        $(fn $method:ident( $($args:tt)* ) $(-> $ret:ty)? { $($body:tt)* })*
    }) => {
        $crate::__paste! {
            impl [<__ $trait_name Impl>] for $type {
                $(
                    #[inline]
                    fn [<__spec_ $method>]( $($args)* ) $(-> $ret)? { $($body)* }
                )*
            }
        }
    };
}

/// Declares a devirtualized trait or implements one for a concrete type.
///
/// Available when the `macros` feature is disabled (i.e.,
/// `default-features = false`). When the default `macros` feature is
/// enabled, use the `#[devirt::devirt]` proc-macro attribute instead.
///
/// # Syntax
///
/// ```ignore
/// // Define a trait with hot types
/// devirt::devirt! {
///     pub MyTrait [HotType1, HotType2] {
///         fn method(&self) -> ReturnType;
///         fn mut_method(&mut self, arg: ArgType);
///     }
/// }
///
/// // Implement for a concrete type
/// devirt::devirt! {
///     impl MyTrait for HotType1 {
///         fn method(&self) -> ReturnType { ... }
///         fn mut_method(&mut self, arg: ArgType) { ... }
///     }
/// }
/// ```
#[cfg(not(feature = "macros"))]
#[macro_export]
macro_rules! devirt {
    // Trait definition
    (
        $(#[$meta:meta])*
        $vis:vis trait $name:ident [$($hot:ty),+ $(,)?] {
            $($methods:tt)*
        }
    ) => {
        $crate::__devirt_define! {
            @trait
            $(#[$meta])*
            $vis $name [$($hot),+] {
                $($methods)*
            }
        }
    };

    // Impl block
    (
        impl $trait_name:ident for $type:ty {
            $($methods:tt)*
        }
    ) => {
        $crate::__devirt_define! {
            @impl
            $trait_name for $type {
                $($methods)*
            }
        }
    };
}

// Re-export the proc-macro attribute when the `macros` feature is enabled.
#[cfg(feature = "macros")]
pub use devirt_macros::devirt;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Unit tests for the unsafe primitives — verify the fat pointer layout
// and vtable identity assumptions that underpin dispatch soundness.
// Run by `cargo test -p devirt` and also exercised by `cargo miri test`.
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod primitives {
    extern crate alloc;
    use alloc::boxed::Box;

    struct Hot {
        val: u64,
    }
    struct Also {
        val: u64,
    }
    struct Cold {
        val: u64,
    }

    crate::__devirt_define! {
        @trait
        pub Probe [Hot, Also] {
            fn get(&self) -> u64;
            fn set(&mut self, v: u64);
        }
    }

    crate::__devirt_define! { @impl Probe for Hot {
        fn get(&self) -> u64 { self.val }
        fn set(&mut self, v: u64) { self.val = v; }
    }}

    crate::__devirt_define! { @impl Probe for Also {
        fn get(&self) -> u64 { self.val.wrapping_add(1) }
        fn set(&mut self, v: u64) { self.val = v.wrapping_add(1); }
    }}

    crate::__devirt_define! { @impl Probe for Cold {
        fn get(&self) -> u64 { self.val.wrapping_sub(1) }
        fn set(&mut self, v: u64) { self.val = v.wrapping_sub(1); }
    }}

    /// The fat pointer's first half is the data pointer, second is vtable.
    /// If this ever fails, every unsafe operation in the dispatch shim
    /// becomes unsound and the `size_of` compile-time assertion is
    /// insufficient to detect it.
    #[test]
    fn fat_pointer_is_data_then_vtable() {
        let hot = Hot { val: 42 };
        let dyn_ref: &dyn Probe = &hot;
        let raw = <dyn Probe>::__devirt_raw_parts(dyn_ref);
        let expected_data = core::ptr::from_ref::<Hot>(&hot) as usize;
        assert_eq!(raw[0], expected_data, "data half mismatch");
        assert_ne!(raw[1], 0, "vtable half must be non-null");
    }

    // Vtable identity is a link-time / LTO-time property: the
    // compiler deduplicates vtables for a given `(Type, Trait)` pair
    // within a single compilation unit, and the linker deduplicates
    // across CGUs under LTO. Miri, however, is a *semantic*
    // interpreter — it may allocate a fresh vtable on every `*const
    // T → *const dyn Trait` coercion because the Rust spec does not
    // guarantee vtable uniqueness. Under Miri the dispatch
    // equivalence tests below still pass (the hot-path comparison
    // just always misses and the fallback returns the correct
    // result), but the identity comparisons themselves are skipped.

    /// `vtable_for::<T>()` must be deterministic: repeated calls return
    /// the same address within a single run.
    #[cfg_attr(miri, ignore = "Miri does not guarantee vtable uniqueness")]
    #[test]
    fn vtable_for_is_deterministic() {
        let a = <dyn Probe>::__devirt_vtable_for::<Hot>();
        let b = <dyn Probe>::__devirt_vtable_for::<Hot>();
        assert_eq!(a, b);
    }

    /// Different concrete types must produce different vtables (so the
    /// comparison dispatch cannot ever match the wrong type).
    #[cfg_attr(miri, ignore = "Miri does not guarantee vtable uniqueness")]
    #[test]
    fn distinct_types_have_distinct_vtables() {
        let h = <dyn Probe>::__devirt_vtable_for::<Hot>();
        let a = <dyn Probe>::__devirt_vtable_for::<Also>();
        let c = <dyn Probe>::__devirt_vtable_for::<Cold>();
        assert_ne!(h, a);
        assert_ne!(h, c);
        assert_ne!(a, c);
    }

    /// Vtables are deduplicated by the compiler: two distinct values of
    /// the same type produce the same vtable pointer.
    #[cfg_attr(miri, ignore = "Miri does not guarantee vtable uniqueness")]
    #[test]
    fn same_type_deduplicated_vtable() {
        let a = Hot { val: 1 };
        let b = Hot { val: 2 };
        let a_dyn: &dyn Probe = &a;
        let b_dyn: &dyn Probe = &b;
        let av = <dyn Probe>::__devirt_raw_parts(a_dyn)[1];
        let bv = <dyn Probe>::__devirt_raw_parts(b_dyn)[1];
        assert_eq!(av, bv);
        assert_eq!(av, <dyn Probe>::__devirt_vtable_for::<Hot>());
    }

    /// End-to-end: dispatch through `&dyn Probe` for hot, hot-second,
    /// and cold types returns the same result as a direct call. This is
    /// the main soundness property.
    #[test]
    fn dispatch_equivalence_ref() {
        let h = Hot { val: 10 };
        let a = Also { val: 10 };
        let c = Cold { val: 10 };
        assert_eq!((&h as &dyn Probe).get(), 10);
        assert_eq!((&a as &dyn Probe).get(), 11);
        assert_eq!((&c as &dyn Probe).get(), 9);
    }

    /// Same end-to-end property for `&mut self` — this is the path most
    /// likely to break under Stacked Borrows.
    #[test]
    fn dispatch_equivalence_mut() {
        let mut h = Hot { val: 0 };
        let mut a = Also { val: 0 };
        let mut c = Cold { val: 0 };
        (&mut h as &mut dyn Probe).set(7);
        (&mut a as &mut dyn Probe).set(7);
        (&mut c as &mut dyn Probe).set(7);
        assert_eq!(h.val, 7);
        assert_eq!(a.val, 8);
        assert_eq!(c.val, 6);
    }

    /// Heap-boxed dispatch also works (important because it exercises a
    /// different aliasing pattern than stack references).
    #[test]
    fn dispatch_through_box() {
        let boxed: Box<dyn Probe> = Box::new(Hot { val: 5 });
        assert_eq!(boxed.get(), 5);
    }
}
