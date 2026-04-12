# devirt

Transparent devirtualization for Rust trait objects via **vtable-pointer
comparison**, with `#![no_std]` support. Eliminates the indirect call
entirely on hot paths — up to **3.4× faster** than plain `dyn Trait` on
shuffled hot-dominant collections (see `benches/dispatch.rs::bench_shuffled_mixed`).

## How it works

`devirt` uses **vtable-pointer-comparison dispatch**. At each call site
through `&dyn Trait`, the generated dispatch shim extracts the vtable
pointer from the fat pointer and compares it against compile-time-known
vtable addresses for each hot type. On a match, the data pointer is
reinterpreted as `&HotType` and the method is called directly (fully
inlined) — no vtable lookup, no indirect call. On a miss, the shim falls
through to a single vtable call via the hidden `__spec_*` method.
Callers use plain `dyn Trait` — no wrappers, no special calls, zero API
change at call sites.

## Usage

The default API uses a proc-macro attribute:

```rust
use std::f64::consts::PI;

struct Circle { radius: f64 }
struct Rect { w: f64, h: f64 }
struct Triangle { a: f64, b: f64, c: f64 }

// 1. Define trait — list hot types in the attribute
#[devirt::devirt(Circle, Rect)]
pub trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
    fn scale(&mut self, factor: f64);
}

// 2. Hot type — vtable-cmp match, direct inlined call
#[devirt::devirt]
impl Shape for Circle {
    fn area(&self) -> f64 { PI * self.radius * self.radius }
    fn perimeter(&self) -> f64 { 2.0 * PI * self.radius }
    fn scale(&mut self, factor: f64) { self.radius *= factor; }
}

#[devirt::devirt]
impl Shape for Rect {
    fn area(&self) -> f64 { self.w * self.h }
    fn perimeter(&self) -> f64 { 2.0 * (self.w + self.h) }
    fn scale(&mut self, factor: f64) { self.w *= factor; self.h *= factor; }
}

// 3. Cold type — falls back to vtable
#[devirt::devirt]
impl Shape for Triangle {
    fn area(&self) -> f64 {
        let s = (self.a + self.b + self.c) / 2.0;
        (s * (s - self.a) * (s - self.b) * (s - self.c)).sqrt()
    }
    fn perimeter(&self) -> f64 { self.a + self.b + self.c }
    fn scale(&mut self, factor: f64) {
        self.a *= factor; self.b *= factor; self.c *= factor;
    }
}

// 4. Use — completely normal dyn Trait
fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}
```

### Without proc macros

If you prefer zero proc-macro dependencies, disable the default `macros`
feature:

```toml
[dependencies]
devirt = { version = "0.2", default-features = false }
```

Then use the declarative macro:

```rust
devirt::devirt! {
    pub trait Shape [Circle, Rect] {
        fn area(&self) -> f64;
        fn perimeter(&self) -> f64;
        fn scale(&mut self, factor: f64);
    }
}

devirt::devirt! {
    impl Shape for Circle {
        fn area(&self) -> f64 { PI * self.radius * self.radius }
        fn perimeter(&self) -> f64 { 2.0 * PI * self.radius }
        fn scale(&mut self, factor: f64) { self.radius *= factor; }
    }
}
```

Both APIs produce identical expanded code.

## When to use

Best when a small number of hot types dominate the population (80%+ of trait
objects). Common scenarios:

- **ECS components** — a few entity types make up most of the world
- **AST nodes** — identifiers and literals vastly outnumber rare nodes
- **Widget trees** — text and containers dominate UI layouts

## When not to use

- **Evenly split collections** — no type dominates, so the vtable
  comparisons add overhead without enough hot-path wins to compensate
- **Cold-dominated collections** — most objects are cold types; the extra
  comparisons before vtable fallback make things slower

## Performance characteristics

| Path               | Cost                                                                                                                                                                                                       |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hot type dispatch  | Single `cmp` against a RIP-relative vtable address + direct, inlined method call. **No indirect call.**                                                                                                    |
| Cold type dispatch | Adds ~0.3 ns per hot type over plain vtable dispatch — one `lea + cmp + jne` per hot type before the vtable fallback. Keep the hot list to ≤3 types to keep this overhead below a single cache-miss cycle. |

LTO is **not required** — all dispatch logic expands via macros into the
user's crate, so there are no cross-crate function calls to inline and
vtable deduplication works within a single crate via COMDAT groups. LTO
may still improve performance for other reasons in your project but is not
necessary for devirt to work correctly.

## Benchmarks

### Single dispatch

| Strategy     | Hot type | Cold type |
| ------------ | -------- | --------- |
| **devirt**   | 1.02 ns  | 2.52 ns   |
| Plain vtable | 1.84 ns  | 2.26 ns   |
| Enum match   | 1.80 ns  | 2.18 ns   |

Hot-type dispatch eliminates the indirect call entirely — the 1.02 ns is
an inlined `PI * r * r`. Cold types pay one `cmp + branch` per hot type
before the vtable fallback, adding ~0.3 ns over plain vtable.

### Mixed collection (4 items, 50/50 hot/cold)

| Strategy     | Time     |
| ------------ | -------- |
| **devirt**   | 9.43 ns  |
| Plain vtable | 10.85 ns |
| Enum match   | 9.78 ns  |

### Hot-dominant collection (10 items, 80% hot)

| Strategy     | Time    |
| ------------ | ------- |
| **devirt**   | 17.5 ns |
| Plain vtable | 24.3 ns |
| Enum match   | 19.6 ns |

### Shuffled collection (80% hot, 20% cold, randomized order)

This is the scenario where devirt shines — the CPU's indirect branch
target buffer cannot learn the call pattern, so plain vtable dispatch
pays the full branch misprediction cost on every call.

| Collection size | devirt  | Plain vtable | Speedup |
| --------------- | ------- | ------------ | ------- |
| n=10            | 18.2 ns | 22.9 ns      | 1.3×    |
| n=100           | 166 ns  | 467 ns       | 2.8×    |
| n=1000          | 1.54 µs | 5.31 µs      | 3.4×    |

Notes:

- **Keep the hot list to ≤3 types.** Each hot type adds one `cmp + branch`
  to the cold path, and more than three becomes a net loss on
  cold-dominated workloads.
- **The shuffled benchmark is the most realistic workload.** Fixed-order
  small collections let the CPU's branch target buffer learn the call
  pattern after a few iterations, which masks most of the difference
  between devirt and plain vtable; real programs rarely dispatch over a
  hot repeating sequence.

Run benchmarks yourself:

```bash
cargo bench --bench dispatch
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.
