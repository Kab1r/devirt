# Fuzz & Compile Testing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Exercise all `macro_rules!` arms in `r#trait!` and `r#impl!` via `cargo fuzz` (runtime correctness) and `trybuild` (compile-time correctness).

**Architecture:** Fuzz target defines a devirt trait + plain trait with identical signatures covering all 4 dispatch arms. Arbitrary inputs select type variant and method args; assertions compare devirt output vs plain vtable output bitwise. Trybuild pass tests cover macro invocation patterns; fail tests verify rejection of invalid inputs.

**Tech Stack:** `cargo-fuzz` (libfuzzer-sys + arbitrary), `trybuild`

**Worktree:** `/home/kabir/Documents/devirt/.worktrees/fuzz-testing`

---

### Task 1: cargo fuzz scaffolding and fuzz target

**Files:**
- Create: `fuzz/Cargo.toml`
- Create: `fuzz/fuzz_targets/dispatch.rs`
- Modify: `.gitignore` (add fuzz artifacts)

**Step 1: Add fuzz artifact paths to `.gitignore`**

Append to `.gitignore`:

```
fuzz/target
fuzz/corpus
fuzz/artifacts
fuzz/coverage
```

**Step 2: Create `fuzz/Cargo.toml`**

```toml
[package]
name = "devirt-fuzz"
version = "0.0.0"
publish = false
edition = "2024"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
arbitrary = { version = "1", features = ["derive"] }
devirt = { path = ".." }

[[bin]]
name = "dispatch"
path = "fuzz_targets/dispatch.rs"
doc = false

[workspace]
members = ["."]
```

**Step 3: Create `fuzz/fuzz_targets/dispatch.rs`**

This is the core fuzz target. It defines:
- 3 types: `HotA` (first in witness chain), `HotB` (second), `Cold` (falls through to vtable)
- 7 trait methods covering all 4 dispatch arms + multi-arg + state readers
- Identical method bodies in a plain trait for baseline comparison
- Each type has DIFFERENT arithmetic per method to detect mis-routing

```rust
#![no_main]

use std::cell::Cell;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

// ── Types ───────────────────────────────────────────────────────────────────

struct HotA {
    val: f64,
    trace: Cell<f64>,
}

struct HotB {
    val: f64,
    trace: Cell<f64>,
}

struct Cold {
    val: f64,
    trace: Cell<f64>,
}

// ── Devirtualized trait ─────────────────────────────────────────────────────

devirt::r#trait! {
    pub Dispatch [HotA, HotB] {
        fn compute(&self, x: f64) -> f64;
        fn notify(&self, x: f64);
        fn transform(&mut self, x: f64) -> f64;
        fn reset(&mut self, x: f64);
        fn combine(&self, x: f64, y: f64) -> f64;
        fn val(&self) -> f64;
        fn trace_val(&self) -> f64;
    }
}

devirt::r#impl!(Dispatch for HotA [hot] {
    fn compute(&self, x: f64) -> f64 { self.val + x }
    fn notify(&self, x: f64) { self.trace.set(self.val + x); }
    fn transform(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 1.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val + x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
});

devirt::r#impl!(Dispatch for HotB [hot] {
    fn compute(&self, x: f64) -> f64 { self.val * x }
    fn notify(&self, x: f64) { self.trace.set(self.val * x); }
    fn transform(&mut self, x: f64) -> f64 { self.val *= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 2.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val * x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
});

devirt::r#impl!(Dispatch for Cold {
    fn compute(&self, x: f64) -> f64 { self.val - x }
    fn notify(&self, x: f64) { self.trace.set(self.val - x); }
    fn transform(&mut self, x: f64) -> f64 { self.val -= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 3.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val - x - y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
});

// ── Plain trait (baseline — normal vtable dispatch) ─────────────────────────

trait PlainDispatch {
    fn compute(&self, x: f64) -> f64;
    fn notify(&self, x: f64);
    fn transform(&mut self, x: f64) -> f64;
    fn reset(&mut self, x: f64);
    fn combine(&self, x: f64, y: f64) -> f64;
    fn val(&self) -> f64;
    fn trace_val(&self) -> f64;
}

impl PlainDispatch for HotA {
    fn compute(&self, x: f64) -> f64 { self.val + x }
    fn notify(&self, x: f64) { self.trace.set(self.val + x); }
    fn transform(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 1.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val + x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
}

impl PlainDispatch for HotB {
    fn compute(&self, x: f64) -> f64 { self.val * x }
    fn notify(&self, x: f64) { self.trace.set(self.val * x); }
    fn transform(&mut self, x: f64) -> f64 { self.val *= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 2.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val * x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
}

impl PlainDispatch for Cold {
    fn compute(&self, x: f64) -> f64 { self.val - x }
    fn notify(&self, x: f64) { self.trace.set(self.val - x); }
    fn transform(&mut self, x: f64) -> f64 { self.val -= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 3.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val - x - y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
}

// ── Fuzz input ──────────────────────────────────────────────────────────────

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    variant: u8,
    val: f64,
    x: f64,
    y: f64,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_devirt(variant: u8, val: f64) -> Box<dyn Dispatch> {
    match variant % 3 {
        0 => Box::new(HotA { val, trace: Cell::new(0.0) }),
        1 => Box::new(HotB { val, trace: Cell::new(0.0) }),
        _ => Box::new(Cold { val, trace: Cell::new(0.0) }),
    }
}

fn make_plain(variant: u8, val: f64) -> Box<dyn PlainDispatch> {
    match variant % 3 {
        0 => Box::new(HotA { val, trace: Cell::new(0.0) }),
        1 => Box::new(HotB { val, trace: Cell::new(0.0) }),
        _ => Box::new(Cold { val, trace: Cell::new(0.0) }),
    }
}

fn assert_bits_eq(a: f64, b: f64, msg: &str) {
    assert_eq!(
        a.to_bits(),
        b.to_bits(),
        "{msg}: devirt={a:?} plain={b:?}"
    );
}

// ── Fuzz target ─────────────────────────────────────────────────────────────

fuzz_target!(|input: FuzzInput| {
    // ── &self methods (non-mutating) ────────────────────────────────────
    let devirt = make_devirt(input.variant, input.val);
    let plain = make_plain(input.variant, input.val);

    // @dispatch_ref: &self, single arg -> f64
    assert_bits_eq(
        devirt.compute(input.x),
        plain.compute(input.x),
        "compute",
    );

    // @dispatch_ref: &self, multi-arg -> f64
    assert_bits_eq(
        devirt.combine(input.x, input.y),
        plain.combine(input.x, input.y),
        "combine",
    );

    // @dispatch_void: &self -> ()
    devirt.notify(input.x);
    plain.notify(input.x);
    assert_bits_eq(devirt.trace_val(), plain.trace_val(), "notify trace");

    // ── &mut self methods (mutating — fresh instances) ──────────────────
    let mut devirt_mut = make_devirt(input.variant, input.val);
    let mut plain_mut = make_plain(input.variant, input.val);

    // @dispatch_mut: &mut self -> f64
    assert_bits_eq(
        devirt_mut.transform(input.x),
        plain_mut.transform(input.x),
        "transform",
    );
    assert_bits_eq(devirt_mut.val(), plain_mut.val(), "state after transform");

    // @dispatch_mut_void: &mut self -> ()
    devirt_mut.reset(input.x);
    plain_mut.reset(input.x);
    assert_bits_eq(devirt_mut.val(), plain_mut.val(), "state after reset");
});
```

**Macro arm coverage map:**

| Method | Dispatch arm | Witness chain tested |
|---|---|---|
| `compute` | `@dispatch_ref` | HotA=first, HotB=rest, Cold=fallthrough |
| `notify` | `@dispatch_void` | HotA=first, HotB=rest, Cold=fallthrough |
| `transform` | `@dispatch_mut` | HotA=first, HotB=rest, Cold=fallthrough |
| `reset` | `@dispatch_mut_void` | HotA=first, HotB=rest, Cold=fallthrough |
| `combine` | `@dispatch_ref` (multi-arg) | arg forwarding with 2 args |
| `val` | `@dispatch_ref` (zero extra args) | zero-arg forwarding |
| `trace_val` | `@dispatch_ref` (zero extra args) | zero-arg forwarding |

**Step 4: Build the fuzz target**

Run: `cargo +nightly fuzz build` (from worktree root)
Expected: compiles without errors.

**Step 5: Run the fuzzer briefly**

Run: `cargo +nightly fuzz run dispatch -- -max_total_time=10`
Expected: runs for 10 seconds, no assertion failures.

**Step 6: Commit**

```bash
git add .gitignore fuzz/
git commit -m "Add cargo fuzz target exercising all dispatch arms"
```

---

### Task 2: trybuild pass tests

**Files:**
- Modify: `Cargo.toml` (add trybuild dev-dependency)
- Create: `tests/ui.rs`
- Create: `tests/ui/single_hot.rs`
- Create: `tests/ui/multi_hot.rs`
- Create: `tests/ui/all_arms.rs`
- Create: `tests/ui/multi_arg.rs`
- Create: `tests/ui/pub_trait.rs`

**Step 1: Add trybuild to dev-dependencies**

Add to `Cargo.toml` under `[dev-dependencies]`:

```toml
trybuild = "1"
```

**Step 2: Create `tests/ui.rs`**

```rust
#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/single_hot.rs");
    t.pass("tests/ui/multi_hot.rs");
    t.pass("tests/ui/all_arms.rs");
    t.pass("tests/ui/multi_arg.rs");
    t.pass("tests/ui/pub_trait.rs");
    t.compile_fail("tests/ui/missing_method.rs");
    t.compile_fail("tests/ui/wrong_signature.rs");
}
```

**Step 3: Create `tests/ui/single_hot.rs`**

Exercises: 1 hot type, 1 method (`&self -> T`), hot impl.

```rust
struct Foo {
    val: f64,
}

devirt::r#trait! {
    pub SingleHot [Foo] {
        fn get(&self) -> f64;
    }
}

devirt::r#impl!(SingleHot for Foo [hot] {
    fn get(&self) -> f64 { self.val }
});

fn main() {
    let f: Box<dyn SingleHot> = Box::new(Foo { val: 1.0 });
    assert_eq!(f.get(), 1.0);
}
```

**Step 4: Create `tests/ui/multi_hot.rs`**

Exercises: 3 hot types, recursive witness chain `[A, B, C]`.

```rust
struct A;
struct B;
struct C;

devirt::r#trait! {
    pub MultiHot [A, B, C] {
        fn id(&self) -> u8;
    }
}

devirt::r#impl!(MultiHot for A [hot] {
    fn id(&self) -> u8 { 1 }
});

devirt::r#impl!(MultiHot for B [hot] {
    fn id(&self) -> u8 { 2 }
});

devirt::r#impl!(MultiHot for C [hot] {
    fn id(&self) -> u8 { 3 }
});

fn main() {
    let items: Vec<Box<dyn MultiHot>> = vec![
        Box::new(A),
        Box::new(B),
        Box::new(C),
    ];
    assert_eq!(items[0].id(), 1);
    assert_eq!(items[1].id(), 2);
    assert_eq!(items[2].id(), 3);
}
```

**Step 5: Create `tests/ui/all_arms.rs`**

Exercises: all 4 dispatch arms (`&self` void/non-void, `&mut self` void/non-void), 1 hot + 1 cold.

```rust
struct Hot {
    val: f64,
}

struct ColdType {
    val: f64,
}

devirt::r#trait! {
    pub AllArms [Hot] {
        fn ref_nonvoid(&self, x: f64) -> f64;
        fn ref_void(&self, x: f64);
        fn mut_nonvoid(&mut self, x: f64) -> f64;
        fn mut_void(&mut self, x: f64);
    }
}

devirt::r#impl!(AllArms for Hot [hot] {
    fn ref_nonvoid(&self, x: f64) -> f64 { self.val + x }
    fn ref_void(&self, _x: f64) { }
    fn mut_nonvoid(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn mut_void(&mut self, x: f64) { self.val = x; }
});

devirt::r#impl!(AllArms for ColdType {
    fn ref_nonvoid(&self, x: f64) -> f64 { self.val + x }
    fn ref_void(&self, _x: f64) { }
    fn mut_nonvoid(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn mut_void(&mut self, x: f64) { self.val = x; }
});

fn main() {
    let mut h: Box<dyn AllArms> = Box::new(Hot { val: 1.0 });
    assert_eq!(h.ref_nonvoid(2.0), 3.0);
    h.ref_void(0.0);
    assert_eq!(h.mut_nonvoid(5.0), 6.0);
    h.mut_void(10.0);

    let mut c: Box<dyn AllArms> = Box::new(ColdType { val: 1.0 });
    assert_eq!(c.ref_nonvoid(2.0), 3.0);
    c.ref_void(0.0);
    assert_eq!(c.mut_nonvoid(5.0), 6.0);
    c.mut_void(10.0);
}
```

**Step 6: Create `tests/ui/multi_arg.rs`**

Exercises: methods with 2 arguments, arg forwarding.

```rust
struct Widget {
    x: f64,
    y: f64,
}

devirt::r#trait! {
    pub MultiArg [Widget] {
        fn add(&self, a: f64, b: f64) -> f64;
        fn set(&mut self, a: f64, b: f64);
    }
}

devirt::r#impl!(MultiArg for Widget [hot] {
    fn add(&self, a: f64, b: f64) -> f64 { self.x + a + self.y + b }
    fn set(&mut self, a: f64, b: f64) { self.x = a; self.y = b; }
});

fn main() {
    let mut w: Box<dyn MultiArg> = Box::new(Widget { x: 1.0, y: 2.0 });
    assert_eq!(w.add(3.0, 4.0), 10.0);
    w.set(5.0, 6.0);
    assert_eq!(w.add(0.0, 0.0), 11.0);
}
```

**Step 7: Create `tests/ui/pub_trait.rs`**

Exercises: `pub` visibility, doc attributes on trait and methods.

```rust
struct Inner {
    val: i32,
}

devirt::r#trait! {
    /// A public trait with documentation.
    pub DocTrait [Inner] {
        /// Returns the inner value.
        fn get(&self) -> i32;
    }
}

devirt::r#impl!(DocTrait for Inner [hot] {
    fn get(&self) -> i32 { self.val }
});

fn main() {
    let d: Box<dyn DocTrait> = Box::new(Inner { val: 42 });
    assert_eq!(d.get(), 42);
}
```

**Step 8: Run pass tests only**

Run: `cargo test ui` (from worktree root)
Expected: all 5 pass tests compile and run successfully. The 2 fail tests will fail because `.stderr` files don't exist yet — that's expected.

**Step 9: Commit**

```bash
git add Cargo.toml tests/
git commit -m "Add trybuild pass tests for macro invocation patterns"
```

---

### Task 3: trybuild fail tests

**Files:**
- Create: `tests/ui/missing_method.rs`
- Create: `tests/ui/missing_method.stderr` (auto-generated)
- Create: `tests/ui/wrong_signature.rs`
- Create: `tests/ui/wrong_signature.stderr` (auto-generated)

**Step 1: Create `tests/ui/missing_method.rs`**

Exercises: hot impl that omits a required method. Should fail because `__spec_second` is not implemented.

```rust
struct Foo;

devirt::r#trait! {
    pub TwoMethods [Foo] {
        fn first(&self) -> i32;
        fn second(&self) -> i32;
    }
}

devirt::r#impl!(TwoMethods for Foo [hot] {
    fn first(&self) -> i32 { 1 }
});

fn main() {}
```

**Step 2: Create `tests/ui/wrong_signature.rs`**

Exercises: impl method with wrong argument type. Should fail because `__spec_compute` signature mismatches trait declaration.

```rust
struct Bar;

devirt::r#trait! {
    pub WrongSig [Bar] {
        fn compute(&self, x: f64) -> f64;
    }
}

devirt::r#impl!(WrongSig for Bar [hot] {
    fn compute(&self, x: u32) -> f64 { f64::from(x) }
});

fn main() {}
```

**Step 3: Generate `.stderr` files**

Run: `TRYBUILD=overwrite cargo test ui` (from worktree root)
Expected: trybuild compiles all tests, the pass tests pass, the fail tests fail to compile (as expected), and `.stderr` files are written to `tests/ui/`.

**Step 4: Verify `.stderr` files were created**

Run: `ls tests/ui/*.stderr`
Expected: `tests/ui/missing_method.stderr` and `tests/ui/wrong_signature.stderr` exist.

**Step 5: Run tests again without overwrite**

Run: `cargo test ui`
Expected: all tests pass (pass tests compile, fail tests match their `.stderr`).

**Step 6: Commit**

```bash
git add tests/ui/
git commit -m "Add trybuild compile-fail tests for invalid macro usage"
```

---

### Task 4: Final verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass (0 unit tests, trybuild ui tests pass).

**Step 2: Run clippy**

Run: `cargo clippy --all-targets`
Expected: no warnings or errors.

**Step 3: Run fuzzer one more time**

Run: `cargo +nightly fuzz run dispatch -- -max_total_time=30`
Expected: 30 seconds, no failures.

**Step 4: Commit if any fixes were needed**

If tasks 1-3 were clean, nothing to commit here. Otherwise:

```bash
git add -A
git commit -m "Fix issues found during final verification"
```
