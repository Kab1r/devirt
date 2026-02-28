# Formal Verification for devirt

## Goal

Prove correctness properties of devirt's dispatch mechanism that fuzzing cannot cover — properties that hold for ALL input values and ALL numbers of hot types, not just sampled ones.

## Verification stack

Four layers, each covering what the layer below cannot:

| Layer | Tool | What it proves | Strength |
|---|---|---|---|
| Universal dispatch properties (all N) | Verus | First-match-wins, exhaustive fallback, termination | Mathematical proof, unbounded |
| No UB + equivalence at specific N | Kani | No panics, no UB, devirt == direct call | Exhaustive over all values, bounded N |
| Runtime correctness at scale | cargo fuzz | Same (probabilistic) | Fast, catches edge cases |
| Compile-time macro correctness | trybuild | Accepts/rejects right syntax | Deterministic |

The Verus proof operates on an **abstract model** of the dispatch chain (a loop over a sequence of witnesses). Kani operates on the **concrete macro expansions** at N=1, N=2, N=3. The connection between them is a structural refinement argument: the macro's recursive unrolling is equivalent to the abstract loop with a statically-known length.

## Workspace structure

The project uses a cargo workspace modeled after the polybolos and periodate projects. Verus proofs follow the periodate pattern (`vstd` as a workspace dependency, `[package.metadata.verus]` on verified crates).

```
Cargo.toml                          # workspace root
crates/
  core/                             # published library (crate name: devirt)
    Cargo.toml
    src/lib.rs
    benches/dispatch.rs
    examples/shapes.rs
    tests/
      kani.rs                       # Kani bounded model checking harnesses
      ui.rs                         # trybuild runner
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
  verify/                           # Verus abstract dispatch proofs (crate name: devirt-verify)
    Cargo.toml
    src/
      lib.rs
      dispatch.rs                   # spec + exec + proof functions
  fuzz/                             # cargo fuzz target (excluded from workspace)
    Cargo.toml
    fuzz_targets/
      dispatch.rs
LICENSE-MIT
LICENSE-APACHE
README.md
docs/
  plans/
```

Root `Cargo.toml`:

```toml
[workspace]
members = ["crates/*"]
exclude = ["crates/fuzz"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/Kab1r/devirt"

[workspace.dependencies]
devirt = { path = "crates/core" }
paste = "1"
vstd = "=0.0.0-2026-01-25-0057"
criterion = { version = "0.5", features = ["html_reports"] }
trybuild = "1"

[workspace.lints.rust]
# ... (current lint config moves here)

[workspace.lints.clippy]
# ... (current lint config moves here)
```

`crates/core/Cargo.toml`:

```toml
[package]
name = "devirt"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Transparent devirtualization for Rust trait objects via witness-method dispatch"
readme = "../../README.md"
keywords = ["devirtualization", "vtable", "no-std", "dispatch", "performance"]
categories = ["no-std", "rust-patterns"]

[dependencies]
paste.workspace = true

[dev-dependencies]
criterion.workspace = true
trybuild.workspace = true

[[bench]]
name = "dispatch"
harness = false

[lints]
workspace = true
```

`crates/verify/Cargo.toml`:

```toml
[package]
name = "devirt-verify"
version.workspace = true
edition.workspace = true
publish = false

[dependencies]
vstd.workspace = true

[package.metadata.verus]
verify = true

[lints]
workspace = true
```

## Part 1: Verus abstract dispatch model

### What we're modeling

The macro's dispatch chain has one fundamental pattern regardless of receiver type (`&self`/`&mut self`) or return type (void/non-void):

```
for each hot type in order:
    call witness → if Some(result), return it
after all witnesses → call fallback
```

The four dispatch arms (`@dispatch_ref`, `@dispatch_void`, `@dispatch_mut`, `@dispatch_mut_void`) are all instances of this pattern. The differences between them (how the result is unwrapped, `return` vs `return value`) are syntactic — the control flow is identical.

### Spec: mathematical definition of correct dispatch

Two spec functions define what dispatch **should** do:

- `first_match(witnesses)` — returns the index of the first `Some` in the sequence, or `None` if all are `None`.
- `dispatch_spec(witnesses, fallback)` — returns the value at `first_match`, or `fallback` if no match.

### Exec: abstract loop matching the macro structure

`dispatch_exec` models the macro's unrolled chain as a loop. Its `ensures` clause ties it to the spec:

```
ensures result == dispatch_spec(witnesses@, fallback)
```

The loop invariant tracks:
1. All witnesses before `idx` returned `None`
2. The spec on the remaining suffix equals the spec on the whole sequence

The `decreases witnesses.len() - idx` clause proves termination. Together, these constitute an inductive proof that the loop is correct for a `Vec` of **any length**.

### Properties proven

**Property A — First-match-wins:** If witness `i` returns `Some`, no witness at index `j > i` is ever consulted, and the result is exactly `witnesses[i].unwrap()`.

**Property B — Exhaustive fallback:** If every witness returns `None`, the result is the fallback value. This is the cold-type guarantee — cold types always reach the vtable.

**Property C — Hot dispatch correctness:** If witness `i` is `Some(val)` and all witnesses before it are `None`, then dispatch returns `val`.

All three properties hold for sequences of any length.

### Verus source

File: `crates/verify/src/dispatch.rs`

```rust
use vstd::prelude::*;

verus! {

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SPEC: mathematical definition of correct dispatch
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Index of the first witness that returns Some, if any.
pub closed spec fn first_match(witnesses: Seq<Option<u64>>) -> Option<nat>
    decreases witnesses.len(),
{
    if witnesses.len() == 0 {
        None
    } else if witnesses[0].is_some() {
        Some(0)
    } else {
        match first_match(witnesses.subrange(1, witnesses.len() as int)) {
            Some(i) => Some(i + 1),
            None => None,
        }
    }
}

/// The correct dispatch result.
pub closed spec fn dispatch_spec(witnesses: Seq<Option<u64>>, fallback: u64) -> u64 {
    match first_match(witnesses) {
        Some(i) => witnesses[i as int].unwrap(),
        None => fallback,
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// EXEC: models the macro-generated dispatch chain as a loop
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn dispatch_exec(witnesses: &Vec<Option<u64>>, fallback: u64) -> (result: u64)
    ensures
        result == dispatch_spec(witnesses@, fallback),
{
    let mut idx: usize = 0;
    while idx < witnesses.len()
        invariant
            idx <= witnesses.len(),
            forall|j: int| 0 <= j < idx as int
                ==> witnesses@[j].is_none(),
            dispatch_spec(witnesses@, fallback)
                == dispatch_spec(
                    witnesses@.subrange(idx as int, witnesses@.len() as int),
                    fallback,
                ),
        decreases witnesses.len() - idx,
    {
        if witnesses[idx].is_some() {
            return witnesses[idx].unwrap();
        }
        idx = idx + 1;
    }
    fallback
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PROOF: properties that follow from the spec
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Property A: First-match-wins. All witnesses before the match are None.
pub proof fn first_match_is_earliest(witnesses: Seq<Option<u64>>)
    ensures
        forall|i: nat| first_match(witnesses) == Some(i)
            ==> forall|j: int| 0 <= j < i as int
                ==> witnesses[j].is_none(),
    decreases witnesses.len(),
{
    if witnesses.len() > 0 && witnesses[0].is_none() {
        first_match_is_earliest(
            witnesses.subrange(1, witnesses.len() as int),
        );
    }
}

/// Property B: Exhaustive fallback. If all witnesses are None, result is fallback.
pub proof fn fallback_always_fires(witnesses: Seq<Option<u64>>, fallback: u64)
    requires
        forall|i: int| 0 <= i < witnesses.len()
            ==> witnesses[i].is_none(),
    ensures
        dispatch_spec(witnesses, fallback) == fallback,
    decreases witnesses.len(),
{
    if witnesses.len() > 0 {
        fallback_always_fires(
            witnesses.subrange(1, witnesses.len() as int),
            fallback,
        );
    }
}

/// Property C: Hot type dispatch returns the correct value.
pub proof fn hot_dispatch_correct(
    witnesses: Seq<Option<u64>>,
    fallback: u64,
    hot_idx: nat,
    val: u64,
)
    requires
        (hot_idx as int) < witnesses.len(),
        witnesses[hot_idx as int] == Some(val),
        forall|j: int| 0 <= j < hot_idx as int
            ==> witnesses[j].is_none(),
    ensures
        dispatch_spec(witnesses, fallback) == val,
    decreases witnesses.len(),
{
    if hot_idx > 0 {
        hot_dispatch_correct(
            witnesses.subrange(1, witnesses.len() as int),
            fallback,
            (hot_idx - 1) as nat,
            val,
        );
    }
}

} // verus!
```

## Part 2: Kani bounded model checking

Kani verifies the **concrete macro expansions** — the actual code that `r#trait!` and `r#impl!` generate. Where Verus proves the abstract dispatch loop is correct for all N, Kani proves that the macro's unrolled output at specific N values has no UB and produces results equivalent to direct calls.

### Reuse of fuzz test types

The Kani harnesses reuse the same type definitions and method bodies from the fuzz testing design (`HotA`, `HotB`, `HotC`, `Cold` with `{ val: f64 }` and deterministic arithmetic). This is sufficient because the dispatch chain never inspects field types or values — it only calls witness methods and forwards results. A proof for `HotA { val: f64 }` transfers to any user type with the same method signature shape.

### Trait definitions

Three trait definitions exercise N=1, N=2, N=3 hot types, covering the base case and recursive cases of the macro's `[$first $(, $rest)*]` → `[$($rest),*]` pattern. Each trait covers all 4 dispatch arms:

```rust
// N=1: base case — one witness check, then fallback
devirt::r#trait! {
    pub Trait1 [HotA] {
        fn compute(&self, x: f64) -> f64;
        fn notify(&self, x: f64);
        fn transform(&mut self, x: f64) -> f64;
        fn reset(&mut self, x: f64);
    }
}

// N=2: one recursive step (matches fuzz design)
devirt::r#trait! {
    pub Trait2 [HotA, HotB] {
        fn compute(&self, x: f64) -> f64;
        fn notify(&self, x: f64);
        fn transform(&mut self, x: f64) -> f64;
        fn reset(&mut self, x: f64);
    }
}

// N=3: two recursive steps — confirms the pattern holds beyond N=2
devirt::r#trait! {
    pub Trait3 [HotA, HotB, HotC] {
        fn compute(&self, x: f64) -> f64;
        fn notify(&self, x: f64);
        fn transform(&mut self, x: f64) -> f64;
        fn reset(&mut self, x: f64);
    }
}
```

### Properties verified per harness

For each of the 3 traits x 3-4 types x 4 methods:

**No UB / no panics:** Kani exhaustively checks that calling through `dyn Trait` cannot panic for any `f64` value.

```rust
#[kani::proof]
fn trait2_hot_a_compute_no_panic() {
    let val: f64 = kani::any();
    let x: f64 = kani::any();
    let obj = HotA { val };
    let s: &dyn Trait2 = &obj;
    let _ = s.compute(x);
}
```

**Dispatch equivalence:** The devirt path produces the same result as calling `__spec_*` directly. Uses `to_bits()` for bitwise equality (handles NaN).

```rust
#[kani::proof]
fn trait2_hot_a_compute_equiv() {
    let val: f64 = kani::any();
    let x: f64 = kani::any();
    let obj = HotA { val };
    let direct = __Trait2Impl::__spec_compute(&obj, x);
    let s: &dyn Trait2 = &obj;
    let dispatched = s.compute(x);
    assert!(dispatched.to_bits() == direct.to_bits());
}
```

**Mutation equivalence (for `&mut self` methods):**

```rust
#[kani::proof]
fn trait2_hot_a_transform_equiv() {
    let val: f64 = kani::any();
    let x: f64 = kani::any();
    let mut obj_devirt = HotA { val };
    let mut obj_direct = HotA { val };
    let r1 = (&mut obj_devirt as &mut dyn Trait2).transform(x);
    let r2 = __Trait2Impl::__spec_transform(&mut obj_direct, x);
    assert!(r1.to_bits() == r2.to_bits());
    assert!(obj_devirt.val.to_bits() == obj_direct.val.to_bits());
}
```

### Why N=1, N=2, N=3 is sufficient

The macro recursion has two arms:
- **Recursive:** `[$first $(, $rest)*]` — peel first element, recurse on rest
- **Base:** `[]` — call fallback

N=1 exercises: one recursive step → base. N=2 exercises: recursive → recursive → base. N=3 exercises: recursive → recursive → recursive → base. Together with the Verus proof that the abstract loop is correct for all N, these three concrete expansions establish that the macro's unrolling faithfully implements the abstract model at multiple points.

### File

`crates/core/tests/kani.rs`

## Part 3: Refinement argument

The Verus proof and Kani harnesses verify different things. The connection between them is an argument that the macro-generated code is an instance of the abstract model.

### The claim

For any macro expansion with N hot types, the generated dispatch chain:

```rust
// Generated by @dispatch_ref for [H1, H2, H3]:
if let Some(v) = __try_method_as_h1(self) { return v; }
if let Some(v) = __try_method_as_h2(self) { return v; }
if let Some(v) = __try_method_as_h3(self) { return v; }
__spec_method(self)
```

is semantically equivalent to `dispatch_exec(&vec![w1, w2, w3], fallback)` where `wi` is the result of calling witness `i` and `fallback` is the result of calling `__spec_method`.

### Why the equivalence holds

The macro recursion and the abstract loop have identical structure:

| Macro recursion | Abstract loop iteration |
|---|---|
| `[$first $(, $rest)*]` arm fires | `idx < witnesses.len()`, check `witnesses[idx]` |
| `__try_method_as_first` returns `Some` → `return` | `witnesses[idx].is_some()` → `return` |
| `__try_method_as_first` returns `None` → recurse on `[$($rest),*]` | `witnesses[idx].is_none()` → `idx += 1` |
| `[]` arm fires → call `__spec_method` | `idx == witnesses.len()` → `return fallback` |

The only difference is that the macro statically unrolls what the loop does dynamically. Unrolling does not change control flow semantics — it replaces a loop with a known trip count with a straight-line sequence of the same operations.

### What Kani adds

Kani validates this structural correspondence empirically at N=1, N=2, N=3 by running the actual macro expansions and proving they produce correct results for all input values. If the macro expansion at some N deviated from the abstract model (e.g., a macro arm reordered witnesses, skipped a check, or fell through incorrectly), Kani would find a counterexample at that N.

The combination:
- **Verus** proves: if the code follows the abstract dispatch pattern, it is correct for all N
- **Kani** proves: the macro output follows the abstract dispatch pattern at N=1, 2, 3 (for all input values)
- **Structural argument**: the macro recursion has exactly two arms (peel-first and base-case), so N=1, 2, 3 exercises every arm and transition

This is not a machine-checked refinement proof. A fully formal version would require encoding the macro expansion rules in a proof assistant, which is outside the scope of current Rust verification tools. The combination of Verus + Kani + structural argument provides high confidence without that cost.

## Running verification

```bash
# Verus: prove abstract dispatch model
cd crates/verify && verus src/lib.rs

# Kani: bounded model checking on concrete expansions
cd crates/core && cargo kani

# Fuzz: runtime correctness (see fuzz testing design doc)
cd crates/fuzz && cargo fuzz run dispatch

# Trybuild: compile-time macro correctness
cd crates/core && cargo test
```

## Implementation order

1. Restructure project into cargo workspace (separate task)
2. Set up `crates/core/tests/kani.rs` with type definitions reused from fuzz design
3. Define `Trait1` (N=1), `Trait2` (N=2), `Trait3` (N=3) with all 4 dispatch arms
4. Write no-panic harnesses for all type/trait/method combinations
5. Write equivalence harnesses for all type/trait/method combinations
6. Run `cargo kani` and fix any issues
7. Set up `crates/verify/` with `vstd` dependency
8. Write the spec functions (`first_match`, `dispatch_spec`)
9. Write `dispatch_exec` with loop invariant
10. Write proof functions for properties A, B, C
11. Run `verus` and iterate on invariants until it verifies

Kani first because it has lower setup cost and will catch concrete bugs before investing in the Verus proof. Verus second because it depends on understanding what the correct spec looks like, which the Kani work will inform.
