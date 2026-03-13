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

### The Witness-Method Pattern

The core idea: instead of a vtable, hot types override a per-type "witness" method `__try_X_as_HotType(&self) -> Option<&HotType>` that returns `Some(self)`. Cold types inherit the default returning `None`. Dispatch unrolls into a chain of `if let Some(t) = self.__try_X_as_HotA() { ... } else if let Some(t) = ...` ending in a vtable call to `__spec_method()`. With LTO, the `Option` overhead disappears and hot paths become direct calls.

### `r#trait!` Expansion

For a trait `Foo` with hot types `[A, B]`:
1. Generates hidden inner trait `__FooImpl` with `__spec_*` methods (the actual impls)
2. Generates `__try_foo_as_a(&self) -> Option<&A>` etc. on the public trait — default returns `None`, hot types override with `Some(self as &A)`
3. Generates public trait `Foo` where each method body is a `@dispatch_*` arm
4. Blanket impl: `impl<T: __FooImpl> Foo for T {}`

### `r#impl!` Expansion

For `impl [hot] Foo for A { ... }`:
- Expands to `impl __FooImpl for A { fn __spec_method(...) { ... } }`
- If `[hot]`: also overrides the witness methods for `A`

### Dispatch Arms (inside `src/lib.rs`)

Four arms handle the combinatorics of `&self`/`&mut self` × void/non-void:
- `@dispatch_ref` — `&self`, returns value
- `@dispatch_void` — `&self`, void
- `@dispatch_mut` — `&mut self`, returns value
- `@dispatch_mut_void` — `&mut self`, void

### Verification Layers

- **UI tests** (`tests/ui/`): trybuild compile tests; `.stderr` files capture expected error output for compile-fail cases
- **Kani** (`tests/kani.rs`): bounded model checker proofs for N=1,2,3 hot types
- **Verus** (`crates/verify/`): full functional correctness proofs (Properties A, B, C) for the dispatch spec

## Key Constraints

- `#![no_std]` throughout `crates/core`
- `paste` is a **regular** (not dev) dependency — it's needed at compile time during macro expansion
- Benchmarks require LTO to be meaningful (profile.bench sets `lto = "thin"`)
- Workspace lints are very strict: `deny` on suspicious/complexity/perf/style/cargo/pedantic/nursery clippy groups
