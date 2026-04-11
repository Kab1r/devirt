# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run all tests (includes UI/compile-fail tests via trybuild)
cargo test --workspace --exclude devirt-fuzz

# Run only UI/compile-fail tests
cargo test --test ui -p devirt

# Run Kani bounded model checker (requires cargo-kani)
cargo kani --tests -p devirt

# Run Miri (requires nightly with miri component) — checks that the
# unsafe fat-pointer transmute and &mut dispatch path are sound under
# Stacked Borrows
cargo +nightly miri test -p devirt --lib

# Run benchmarks (LTO required for meaningful numbers)
cargo bench

# Run example
cargo run --example shapes

# Lint
cargo clippy --workspace --exclude devirt-fuzz --all-targets -- -D warnings

# Fuzz (requires nightly + cargo-fuzz)
cargo +nightly fuzz run dispatch

# Verus formal verification (requires verus on PATH)
verus crates/verify/src/lib.rs --crate-type=lib
```

## Architecture

This is a workspace with three crates:

- **`crates/core`** (`devirt`) — the main proc-macro-free macro library. Two public macros: `r#trait!` and `r#impl!`.
- **`crates/verify`** (`devirt-verify`) — Verus formal proofs of dispatch correctness.
- **`fuzz`** — libfuzzer differential fuzzing comparing devirt dispatch vs. plain vtable.

### The Vtable-Pointer Comparison Pattern

The core idea: the generated dispatch shim lives in an inherent `impl dyn Trait { ... }` block (not as a trait default method). For each call, it extracts the `[data, vtable]` halves of the fat pointer via `transmute::<&dyn Trait, [usize; 2]>` and compares the vtable half against the compile-time-known vtable for each hot type (obtained by coercing a dangling `*const HotType` to `*const dyn Trait`). On match, the data pointer is reinterpreted as `&HotType` and the concrete type's `__spec_*` method is called directly (fully inlined under LTO). On a full miss, the shim falls through to a single vtable call via the inner trait's `__spec_*` method.

Why inherent methods on `dyn Trait` and not trait default methods: a default method body cannot cast `self as *const dyn Trait` because `Self: ?Sized` at that point. Inherent impls on `dyn Trait` have `Self = dyn Trait` directly, so the cast is just a ref-to-pointer conversion.

### `r#trait!` Expansion

For a trait `Foo` with hot types `[A, B]`:
1. Generates hidden inner trait `__FooImpl` with `__spec_*` method declarations — user types provide the bodies via `r#impl!`.
2. Generates a compile-time assertion that `size_of::<*const dyn Foo>() == 2 * size_of::<usize>()`.
3. Generates `impl<'a> dyn Foo + 'a { ... }` with two primitive helpers — `__devirt_raw_parts(&Self) -> [usize; 2]` and `__devirt_vtable_for::<T: __FooImpl + 'static>() -> usize` — plus inherent methods for each user-declared trait method whose body is the vtable-comparison dispatch shim.
4. Generates public marker trait `Foo: __FooImpl` (no methods of its own).
5. Blanket impl: `impl<T: __FooImpl + ?Sized> Foo for T {}`.

### `r#impl!` Expansion

For `impl [hot] Foo for A { ... }` or `impl Foo for A { ... }`:
- Expands to `impl __FooImpl for A { fn __spec_method(...) { ... } }`.
- The `[hot]` marker is accepted for backward compatibility but is purely documentary: hot-path specialization is driven entirely by the trait's hot-type list in `r#trait!`, not by per-impl overrides.

### Dispatch Arms (inside `src/lib.rs`)

Four arms handle the combinatorics of `&self`/`&mut self` × void/non-void. Each splits into an outer "set up `__raw`" arm and a recursive `*_chain` arm that walks the hot-type list:
- `@dispatch_ref` / `@dispatch_ref_chain` — `&self`, returns value
- `@dispatch_void` / `@dispatch_void_chain` — `&self`, void
- `@dispatch_mut` / `@dispatch_mut_chain` — `&mut self`, returns value
- `@dispatch_mut_void` / `@dispatch_mut_void_chain` — `&mut self`, void

Recursive expansion (rather than `$()+` repetition) avoids macro_rules metavar depth conflicts between `$hot` and `$arg`.

### Verification Layers

- **Miri** (`cargo +nightly miri test -p devirt --lib`): runs the `#[cfg(test)] mod primitives` harnesses — fat pointer layout, vtable identity, and end-to-end `&self` / `&mut self` / `Box<dyn>` dispatch — under Stacked/Tree Borrows to catch aliasing violations in the unsafe transmute and `&mut` paths.
- **UI tests** (`tests/ui/`): trybuild compile tests; `.stderr` files capture expected error output for compile-fail cases.
- **Kani** (`tests/kani.rs`): bounded model checker proofs for N=1,2,3 hot types, plus a `mod vt` section that directly verifies vtable-primitive soundness.
- **Verus** (`crates/verify/`): full functional correctness proofs (Properties A, B, C) for the abstract dispatch spec, plus a `vtable_refines_witness` refinement lemma tying the new `vtable_dispatch_spec` to the existing `dispatch_spec`.

## Key Constraints

- `#![no_std]` throughout `crates/core`
- `paste` is a **regular** (not dev) dependency — it's needed at compile time during macro expansion
- Benchmarks require LTO to be meaningful (profile.bench sets `lto = "thin"`)
- Workspace lints are very strict: `deny` on suspicious/complexity/perf/style/cargo/pedantic/nursery clippy groups. `unsafe_code` is `deny` (not `forbid`) so `crates/core` can locally `#![allow(unsafe_code)]` — all other crates still disallow unsafe.
