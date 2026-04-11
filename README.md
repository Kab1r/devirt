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
inlined under LTO) — no vtable lookup, no indirect call. On a miss, the
shim falls through to a single vtable call via the hidden `__spec_*`
method. Callers use plain `dyn Trait` — no wrappers, no special calls,
zero API change at call sites.

## LTO required

This crate relies on cross-function inlining **and** cross-CGU vtable
deduplication. **Without LTO, the vtable-comparison may always miss
(because the trait and the hot type's impl live in different codegen units
and their vtables are not deduplicated), silently degrading to plain
`dyn Trait` dispatch.**

Add this to your `Cargo.toml`:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
```

## Usage

```rust
use devirt;

// 1. Define trait — list hot types in brackets
devirt::r#trait! {
    pub Shape [Circle, Rect] {
        fn area(&self) -> f64;
        fn perimeter(&self) -> f64;
        fn scale(&mut self, factor: f64);
    }
}

// 2. Hot type — vtable-cmp match, direct inlined call under LTO
devirt::r#impl!(Shape for Circle [hot] {
    fn area(&self) -> f64 {
        core::f64::consts::PI * self.radius * self.radius
    }
    fn perimeter(&self) -> f64 {
        2.0 * core::f64::consts::PI * self.radius
    }
    fn scale(&mut self, factor: f64) {
        self.radius *= factor;
    }
});

// 3. Cold type — falls back to vtable
devirt::r#impl!(Shape for Triangle {
    fn area(&self) -> f64 { /* ... */ }
    fn perimeter(&self) -> f64 { /* ... */ }
    fn scale(&mut self, factor: f64) { /* ... */ }
});

// 4. Use — completely normal dyn Trait
fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}
```

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

| Path               | Cost                                                                                                                                                                                                        |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hot type dispatch  | Single `cmp` against a RIP-relative vtable address + direct, inlined method call under LTO. **No indirect call.**                                                                                           |
| Cold type dispatch | Adds ~0.3 ns per hot type over plain vtable dispatch — one `lea + cmp + jne` per hot type before the vtable fallback. Keep the hot list to ≤3 types to keep this overhead below a single cache-miss cycle.  |

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

- **LTO is required.** Without it, the trait and the hot type's impl may
  end up in different codegen units and their vtables may not be
  deduplicated, so the comparison always misses and dispatch silently
  degrades to plain vtable.
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
# With LTO (default)
cargo bench --bench dispatch

# Without LTO (to stress-test)
RUSTFLAGS="-C lto=off -C codegen-units=256" cargo bench --bench dispatch
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.
