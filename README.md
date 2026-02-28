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

| Path | Cost |
|------|------|
| Hot type dispatch | Zero overhead vs direct call (with LTO) |
| Cold type dispatch | Linear in the number of hot types (one inlined `None`-returning branch per hot type before vtable fallback) |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.
