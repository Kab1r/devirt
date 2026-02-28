# Fuzz and Compile Testing for devirt

## Goal

Exercise all `macro_rules!` arms in `r#trait!` and `r#impl!` through two complementary strategies:

1. **`cargo fuzz`** — runtime correctness: statically instantiate every macro arm, then fuzz the runtime values flowing through them. Assert devirtualized dispatch always matches plain vtable dispatch.
2. **`trybuild`** — compile-time correctness: verify diverse valid invocations compile, and invalid invocations produce errors.

## Macro arms to cover

`r#trait!` dispatch arms (4):
- `@dispatch_ref` — `&self`, non-void return
- `@dispatch_void` — `&self`, void return
- `@dispatch_mut` — `&mut self`, non-void return
- `@dispatch_mut_void` — `&mut self`, void return

Each dispatch arm also exercises:
- `@spec_decl` (method declaration in inner trait)
- `@witness_defaults` (default `None` witness per hot type)
- `@outer_decl` (public trait method with dispatch chain)

`r#impl!` arms:
- Cold impl (`@hot_spec` not used, just `__spec_*`)
- Hot impl — `@hot_spec` + `@hot_witness` for each receiver/return combo

Witness chain coverage:
- 2 hot types exercises the recursive `[$first $(, $rest)*]` → `[$($rest),*]` chain
- 1 cold type exercises the base case (falls through all witnesses to `__spec_*`)

## Part 1: `cargo fuzz` target

### Trait design

One devirt trait and one plain trait with identical signatures covering all 4 arms plus multi-arg forwarding:

```rust
// All four dispatch arms + multi-arg variant
fn compute(&self, x: f64) -> f64;             // @dispatch_ref
fn notify(&self, x: f64);                      // @dispatch_void
fn transform(&mut self, x: f64) -> f64;        // @dispatch_mut
fn reset(&mut self, x: f64);                   // @dispatch_mut_void
fn combine(&self, x: f64, y: f64) -> f64;      // @dispatch_ref, multi-arg
```

### Types

- `HotA { val: f64 }` — first hot type (first in witness chain)
- `HotB { val: f64 }` — second hot type (rest in witness chain)
- `Cold { val: f64 }` — cold type (falls through to vtable)

All method bodies use simple deterministic arithmetic on `self.val` and the arguments so we can assert exact bitwise equality.

### Fuzz input

```rust
#[derive(Debug, Arbitrary)]
struct FuzzInput {
    variant: u8,   // 0=HotA, 1=HotB, 2=Cold
    val: f64,      // struct field
    x: f64,        // first method arg
    y: f64,        // second method arg (for combine)
}
```

### Assertion strategy

For each fuzzed input:
1. Construct both `dyn DevirtTrait` and `dyn PlainTrait` with the same variant and field value
2. Call all five methods on both
3. For non-void methods: assert `result_devirt.to_bits() == result_plain.to_bits()` (handles NaN)
4. For void `&mut self` methods: assert the mutated field values are bitwise-equal after the call
5. For void `&self` methods: use a side-channel (return from inner spec, checked via witness) — or simply verify no panic, since the void `&self` path is structurally identical to the non-void path minus the return value

### File

`fuzz/fuzz_targets/dispatch.rs`

## Part 2: `trybuild` tests

### Pass tests (should compile)

| File | What it exercises |
|---|---|
| `single_hot.rs` | 1 hot type, 1 method (`&self -> T`) |
| `multi_hot.rs` | 3 hot types, 1 method |
| `all_arms.rs` | 1 hot + 1 cold, all 4 method signatures |
| `multi_arg.rs` | Methods with 2+ arguments |
| `pub_trait.rs` | `pub` visibility, doc attributes on trait and methods |

### Fail tests (should produce compile errors)

| File | What it catches |
|---|---|
| `missing_method.rs` | Impl block omits a required method |
| `wrong_signature.rs` | Impl method has wrong arg types |

`.stderr` files will be auto-generated on first run with `TRYBUILD=overwrite` and checked in. They are fragile across Rust versions — regenerate when bumping MSRV.

## File layout

```
fuzz/
  Cargo.toml
  fuzz_targets/
    dispatch.rs
tests/
  ui.rs
  ui/
    single_hot.rs
    multi_hot.rs
    all_arms.rs
    multi_arg.rs
    pub_trait.rs
    missing_method.rs
    missing_method.stderr
    wrong_signature.rs
    wrong_signature.stderr
```

## Dependencies

- `fuzz/Cargo.toml`: `libfuzzer-sys` with `arbitrary` feature, path dep on `devirt`
- Main `Cargo.toml` `[dev-dependencies]`: `trybuild = "1"`

## Implementation order

1. Set up `cargo fuzz` scaffolding (`fuzz/Cargo.toml`, target file)
2. Write the devirt + plain trait definitions and impl blocks in the fuzz target
3. Write the fuzz harness with `Arbitrary` input and bitwise assertions
4. Run `cargo fuzz run dispatch` to verify it works
5. Add `trybuild` dev-dependency
6. Write `tests/ui.rs` runner
7. Write pass test cases
8. Write fail test cases, generate `.stderr` files
9. Run `cargo test` to verify trybuild passes
