//! Transparent devirtualization for Rust trait objects, with `#![no_std]` support.
//!
//! Hot types get witness-method dispatch: a thin inlined check routes directly
//! to the concrete type's method, avoiding the vtable.
//! Cold types fall back to normal vtable dispatch.
//! Callers use plain `dyn Trait` — no wrappers, no special calls.
//!
//! # Architecture
//!
//! `r#trait!` generates:
//! - A hidden inner trait `__XImpl` with `__spec_*` methods and
//!   `__try_*_as_*` witness methods (one default per hot type per method)
//! - A public trait `X` with devirtualized default methods
//! - A blanket impl: `impl<T: __XImpl> X for T {}`
//!
//! `r#impl!` generates:
//! - `impl __XImpl for ConcreteType { ... }` with the `__spec_*` bodies
//! - When marked `[hot]`, also overrides the matching witness methods so
//!   dispatch bypasses the vtable for this type
//!
//! # Usage
//!
//! ```ignore
//! devirt::r#trait! {
//!     pub MyTrait [HotType1, HotType2] {
//!         /// Doc for the method.
//!         fn method(&self) -> ReturnType;
//!     }
//! }
//!
//! // Hot type: witness override — one direct call, no vtable
//! devirt::r#impl!(MyTrait for HotType1 [hot] {
//!     fn method(&self) -> ReturnType { ... }
//! });
//!
//! // Cold type: falls back to vtable
//! devirt::r#impl!(MyTrait for ColdType {
//!     fn method(&self) -> ReturnType { ... }
//! });
//! ```
//!
//! # Required: enable LTO
//!
//! This crate relies on cross-function inlining to eliminate dispatch overhead.
//! Without LTO, witness methods may not inline and performance will be **worse**
//! than plain `dyn Trait`.
//!
//! ```toml
//! [profile.release]
//! lto = "thin"
//! codegen-units = 1
//! ```
//!
//! # Performance characteristics
//!
//! With LTO enabled, hot-type dispatch compiles to the same machine code as a
//! direct method call — zero overhead vs plain `dyn Trait`. Cold types pay a
//! small penalty: each hot type adds one inlined branch (returning `None`) to
//! the cold path before the vtable fallback. This means cold-path overhead
//! grows linearly with the number of hot types.
//!
//! The crate is most effective when hot types dominate the population (80%+
//! of trait objects). In mixed collections with many cold types, the cold-path
//! penalty can outweigh the hot-path gains.
//!
//! # Limitations
//!
//! **Object safety required.** All trait methods must be object-safe — no
//! generic parameters, no `Self` in return position. The generated blanket
//! impl requires `dyn Trait` to work, so violating object safety will produce
//! errors pointing at the generated impl rather than the method definition.
//!
//! **Snake-case name collisions.** Hot type names are converted to `snake_case`
//! (via `paste`'s `:snake`) for witness method names. Two types that produce
//! the same `snake_case` form (e.g., `HTTPClient` and `HttpClient` both become
//! `http_client`) will generate conflicting method names. Use distinct type
//! names when this would be ambiguous.

#![no_std]

#[doc(hidden)]
pub use paste::paste as __paste;

/// Declares a trait with transparent devirtualization.
///
/// Hot types listed in brackets get witness-method dispatch (single vtable call).
/// Cold types fall back to normal vtable dispatch.
/// Callers use plain `dyn Trait` — no wrappers, no special calls.
///
/// # Syntax
///
/// ```ignore
/// devirt::r#trait! {
///     pub MyTrait [HotType1, HotType2] {
///         /// Doc comment forwarded to the generated trait method.
///         fn method(&self) -> ReturnType;
///         fn mut_method(&mut self, arg: ArgType);
///     }
/// }
/// ```
///
/// # Notes
///
/// Hot types must be simple, unqualified type names (e.g., `Circle`, not
/// `crate::Circle`).
#[macro_export]
macro_rules! r#trait {
    (
        $(#[$meta:meta])*
        $vis:vis $trait_name:ident [$($hot:ty),+ $(,)?] {
            $($methods:tt)*
        }
    ) => {
        $crate::__paste! {
            #[doc(hidden)]
            $vis trait [<__ $trait_name Impl>] {
                $crate::r#trait!{@spec_decl $($methods)*}
                $crate::r#trait!{@all_witness_defaults [$($hot),+], $($methods)*}
            }

            $(#[$meta])*
            $vis trait $trait_name: [<__ $trait_name Impl>] {
                $crate::r#trait!{@outer_decl [<__ $trait_name Impl>], [$($hot),+], $($methods)*}
            }

            impl<T: [<__ $trait_name Impl>]> $trait_name for T {}
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
        $crate::r#trait!{@spec_decl $($rest)*}
    };

    (@spec_decl
        $(#[$_attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            fn [<__spec_ $method>](&mut self $(, $arg: $argty)*) $(-> $ret)?;
        }
        $crate::r#trait!{@spec_decl $($rest)*}
    };

    (@spec_decl) => {};

    // ── @all_witness_defaults ───────────────────────────────────────────────

    (@all_witness_defaults [$first:ty $(, $rest:ty)*], $($methods:tt)*) => {
        $crate::r#trait!{@witness_defaults $first, $($methods)*}
        $crate::r#trait!{@all_witness_defaults [$($rest),*], $($methods)*}
    };

    (@all_witness_defaults [], $($methods:tt)*) => {};

    // ── @witness_defaults ───────────────────────────────────────────────────

    // &self with explicit return type
    (@witness_defaults $hot:ty,
        $(#[$_attr:meta])*
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*) -> $ret:ty;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $hot:snake>](&self $(, _ : $argty)*) -> Option<$ret> { None }
        }
        $crate::r#trait!{@witness_defaults $hot, $($rest)*}
    };

    // &self without return type
    (@witness_defaults $hot:ty,
        $(#[$_attr:meta])*
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*);
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $hot:snake>](&self $(, _ : $argty)*) -> Option<()> { None }
        }
        $crate::r#trait!{@witness_defaults $hot, $($rest)*}
    };

    // &mut self with explicit return type
    (@witness_defaults $hot:ty,
        $(#[$_attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*) -> $ret:ty;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $hot:snake>](&mut self $(, _ : $argty)*) -> Option<$ret> { None }
        }
        $crate::r#trait!{@witness_defaults $hot, $($rest)*}
    };

    // &mut self without return type
    (@witness_defaults $hot:ty,
        $(#[$_attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*);
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $hot:snake>](&mut self $(, _ : $argty)*) -> Option<()> { None }
        }
        $crate::r#trait!{@witness_defaults $hot, $($rest)*}
    };

    (@witness_defaults $hot:ty,) => {};

    // ── @outer_decl ─────────────────────────────────────────────────────────

    // &self, non-void
    (@outer_decl $inner:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*) -> $ret:ty;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            $(#[$attr])*
            #[inline]
            fn $method(&self $(, $arg: $argty)*) -> $ret {
                $crate::r#trait!(@dispatch_ref $inner, self, $method, ($($arg),*), [$($hot),+])
            }
        }
        $crate::r#trait!{@outer_decl $inner, [$($hot),+], $($rest)*}
    };

    // &self, void
    (@outer_decl $inner:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*);
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            $(#[$attr])*
            #[inline]
            fn $method(&self $(, $arg: $argty)*) {
                $crate::r#trait!(@dispatch_void $inner, self, $method, ($($arg),*), [$($hot),+])
            }
        }
        $crate::r#trait!{@outer_decl $inner, [$($hot),+], $($rest)*}
    };

    // &mut self, non-void
    (@outer_decl $inner:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*) -> $ret:ty;
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            $(#[$attr])*
            #[inline]
            fn $method(&mut self $(, $arg: $argty)*) -> $ret {
                $crate::r#trait!(@dispatch_mut $inner, self, $method, ($($arg),*), [$($hot),+])
            }
        }
        $crate::r#trait!{@outer_decl $inner, [$($hot),+], $($rest)*}
    };

    // &mut self, void
    (@outer_decl $inner:ident, [$($hot:ty),+],
        $(#[$attr:meta])*
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*);
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            $(#[$attr])*
            #[inline]
            fn $method(&mut self $(, $arg: $argty)*) {
                $crate::r#trait!(@dispatch_mut_void $inner, self, $method, ($($arg),*), [$($hot),+])
            }
        }
        $crate::r#trait!{@outer_decl $inner, [$($hot),+], $($rest)*}
    };

    (@outer_decl $inner:ident, [$($hot:ty),+],) => {};

    // ── @dispatch_ref: &self, non-void ─────────────────────────────────────

    (@dispatch_ref $inner:ident, $this:tt, $method:ident, ($($arg:expr),*),
        [$first:ty $(, $rest:ty)*]
    ) => {{
        if let Some(__devirt_v) =
            $crate::__paste! { $inner::[<__try_ $method _as_ $first:snake>] }($this $(, $arg)*)
        {
            return __devirt_v;
        }
        $crate::r#trait!(@dispatch_ref $inner, $this, $method, ($($arg),*), [$($rest),*])
    }};

    (@dispatch_ref $inner:ident, $this:tt, $method:ident, ($($arg:expr),*), []) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }($this $(, $arg)*)
    };

    // ── @dispatch_void: &self, void ─────────────────────────────────────────

    (@dispatch_void $inner:ident, $this:tt, $method:ident, ($($arg:expr),*),
        [$first:ty $(, $rest:ty)*]
    ) => {{
        if $crate::__paste! { $inner::[<__try_ $method _as_ $first:snake>] }($this $(, $arg)*).is_some() {
            return;
        }
        $crate::r#trait!(@dispatch_void $inner, $this, $method, ($($arg),*), [$($rest),*])
    }};

    (@dispatch_void $inner:ident, $this:tt, $method:ident, ($($arg:expr),*), []) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }($this $(, $arg)*)
    };

    // ── @dispatch_mut: &mut self, non-void ─────────────────────────────────

    (@dispatch_mut $inner:ident, $this:tt, $method:ident, ($($arg:expr),*),
        [$first:ty $(, $rest:ty)*]
    ) => {{
        if let Some(__devirt_v) =
            $crate::__paste! { $inner::[<__try_ $method _as_ $first:snake>] }(&mut *$this $(, $arg)*)
        {
            return __devirt_v;
        }
        $crate::r#trait!(@dispatch_mut $inner, $this, $method, ($($arg),*), [$($rest),*])
    }};

    (@dispatch_mut $inner:ident, $this:tt, $method:ident, ($($arg:expr),*), []) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }(&mut *$this $(, $arg)*)
    };

    // ── @dispatch_mut_void: &mut self, void ────────────────────────────────

    (@dispatch_mut_void $inner:ident, $this:tt, $method:ident, ($($arg:expr),*),
        [$first:ty $(, $rest:ty)*]
    ) => {{
        if $crate::__paste! { $inner::[<__try_ $method _as_ $first:snake>] }(&mut *$this $(, $arg)*).is_some() {
            return;
        }
        $crate::r#trait!(@dispatch_mut_void $inner, $this, $method, ($($arg),*), [$($rest),*])
    }};

    (@dispatch_mut_void $inner:ident, $this:tt, $method:ident, ($($arg:expr),*), []) => {
        $crate::__paste! { $inner::[<__spec_ $method>] }(&mut *$this $(, $arg)*)
    };
}

/// Implements a devirtualized trait for a concrete type.
///
/// Use `[hot]` when this type is listed as a hot type in the `r#trait!`
/// declaration. Hot types override the witness methods so dispatch skips
/// the vtable. Cold types fall back to vtable-based dispatch.
///
/// # Syntax
///
/// ```ignore
/// // Hot type (must be listed in the trait's `[...]` hot list):
/// devirt::r#impl!(MyTrait for HotType [hot] {
///     fn method(&self) -> ReturnType { ... }
/// });
///
/// // Cold type (not in the hot list):
/// devirt::r#impl!(MyTrait for ColdType {
///     fn method(&self) -> ReturnType { ... }
/// });
/// ```
///
/// # Notes
///
/// The type name in `[hot]` impls must be the same simple, unqualified name
/// used in the `r#trait!` hot list.
#[macro_export]
macro_rules! r#impl {
    // Cold impl
    ($trait_name:ident for $type:ty {
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

    // Hot impl
    ($trait_name:ident for $type:ty [hot] {
        $($methods:tt)*
    }) => {
        $crate::__paste! {
            impl [<__ $trait_name Impl>] for $type {
                $crate::r#impl!{@hot_spec $($methods)*}
                $crate::r#impl!{@hot_witness $type, $($methods)*}
            }
        }
    };

    // ── @hot_spec ───────────────────────────────────────────────────────────

    (@hot_spec
        fn $method:ident( $($args:tt)* ) $(-> $ret:ty)? { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__spec_ $method>]( $($args)* ) $(-> $ret)? { $($body)* }
        }
        $crate::r#impl!{@hot_spec $($rest)*}
    };

    (@hot_spec) => {};

    // ── @hot_witness ────────────────────────────────────────────────────────

    // &self with return type
    (@hot_witness $type:ty,
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*) -> $ret:ty { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $type:snake>](&self $(, $arg: $argty)*) -> Option<$ret> {
                Some(self.[<__spec_ $method>]($($arg),*))
            }
        }
        $crate::r#impl!{@hot_witness $type, $($rest)*}
    };

    // &self without return type
    (@hot_witness $type:ty,
        fn $method:ident(&self $(, $arg:ident : $argty:ty)*) { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $type:snake>](&self $(, $arg: $argty)*) -> Option<()> {
                self.[<__spec_ $method>]($($arg),*);
                Some(())
            }
        }
        $crate::r#impl!{@hot_witness $type, $($rest)*}
    };

    // &mut self with return type
    (@hot_witness $type:ty,
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*) -> $ret:ty { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $type:snake>](&mut self $(, $arg: $argty)*) -> Option<$ret> {
                Some(self.[<__spec_ $method>]($($arg),*))
            }
        }
        $crate::r#impl!{@hot_witness $type, $($rest)*}
    };

    // &mut self without return type
    (@hot_witness $type:ty,
        fn $method:ident(&mut self $(, $arg:ident : $argty:ty)*) { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::__paste! {
            #[inline]
            fn [<__try_ $method _as_ $type:snake>](&mut self $(, $arg: $argty)*) -> Option<()> {
                self.[<__spec_ $method>]($($arg),*);
                Some(())
            }
        }
        $crate::r#impl!{@hot_witness $type, $($rest)*}
    };

    (@hot_witness $type:ty,) => {};
}
