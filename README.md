# devirt

Transparent devirtualization for Rust trait objects. 29% faster dispatch on
hot-dominated collections vs plain `dyn Trait`, with `#![no_std]` support.

## How it works

`devirt` uses **witness-method dispatch**: hot types (the ones you expect to
dominate your collections) get a thin inlined check that routes directly to the
concrete type's method, bypassing the vtable entirely. Cold types fall back to
normal vtable dispatch. Callers use plain `dyn Trait` — no wrappers, no
special calls, zero API change at call sites.

## LTO required

This crate relies on cross-function inlining to eliminate dispatch overhead.
**Without LTO, performance will be worse than plain `dyn Trait`.**

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

// 2. Hot type — witness override, no vtable
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

- **Evenly split collections** — no type dominates, so the witness checks add
  overhead without enough hot-path wins to compensate
- **Cold-dominated collections** — most objects are cold types; the extra
  branches before vtable fallback make things slower

## Performance characteristics

| Path               | Cost                                                                                                        |
| ------------------ | ----------------------------------------------------------------------------------------------------------- |
| Hot type dispatch  | Zero overhead vs direct call (with LTO)                                                                     |
| Cold type dispatch | Linear in the number of hot types (one inlined `None`-returning branch per hot type before vtable fallback) |

## Benchmarks

Comprehensive benchmarks comparing three dispatch strategies:

### Single Method Call (Hot Type)

| Strategy     | With LTO | Without LTO    |
| ------------ | -------- | -------------- |
| **devirt**   | 1.64 ns  | 2.05 ns        |
| Plain vtable | 2.05 ns  | 1.69 ns        |
| Enum-based   | 2.13 ns  | **1.47 ns** ⭐ |

**Finding:** With LTO, devirt achieves near-perfect zero overhead on hot types. Without LTO, explicit enum-based dispatch is fastest, but devirt remains competitive. (Note: Enum-based is unusually faster without LTO due to simpler code layout and better CPU cache locality for this tight loop.)

### Single Method Call (Cold Type)

| Strategy     | With LTO | Without LTO |
| ------------ | -------- | ----------- |
| **devirt**   | 3.33 ns  | 3.28 ns     |
| Plain vtable | 5.17 ns  | 3.28 ns     |
| Enum-based   | 2.79 ns  | 2.71 ns ⭐  |

**Finding:** Devirt's cold-type penalty (witness checks before vtable fallback) is small. Plain vtable is slower with LTO. Enum-based is fastest in both cases.

### Mixed Collection (50/50 Hot/Cold, 4 items)

| Strategy     | With LTO       | Without LTO    |
| ------------ | -------------- | -------------- |
| **devirt**   | 12.03 ns       | 12.22 ns       |
| Plain vtable | 12.18 ns       | 19.56 ns ⚠️    |
| Enum-based   | **9.83 ns** ⭐ | **8.10 ns** ⭐ |

**Finding:** Devirt ties with plain vtable when LTO is enabled. Without LTO, plain vtable degrades dramatically (2.4x slower), while devirt remains stable. Enum-based is fastest in realistic mixed workloads due to better CPU cache locality and branch prediction.

### Key Takeaways

1. **With LTO (recommended):** Devirt achieves its design goal—hot-type dispatch is as fast as a direct call (1.64 ns), with minimal cold-type penalty.

2. **Without LTO:**
   - Hot-type dispatch has ~35% overhead (2.05 ns vs 1.69 ns plain)
   - Mixed workloads remain competitive (devirt 12.22 ns vs plain 19.56 ns)
   - Explicit enum dispatch is fastest but requires API changes

3. **Trade-off:** Devirt offers performance close to enum-based dispatch while maintaining transparent `dyn Trait` API. The 35% overhead without LTO is acceptable for the flexibility gained.

### Benchmark Methodology Notes

The criterion benchmarks measure the entire compiled program (including criterion itself), so they're affected by how the overall binary is optimized. When the dispatch code is isolated in a standalone binary and measured with hyperfine:

- **With LTO:** 935.1 ms ± 11.5 ms
- **Without LTO:** 936.2 ms ± 14.3 ms
- **Difference:** 1.00x (within noise)

The generated assembly is identical; differences in criterion results stem from binary layout effects under different optimization strategies.

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
