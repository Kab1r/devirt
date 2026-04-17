# Real-World Devirt Benchmarks

Benchmarks measuring `devirt`'s impact on the `tracing::Subscriber` dispatch
pattern — the most heavily-used `dyn Trait` dispatch in the Rust ecosystem.

## Results Summary

### Microbenchmarks (`tracing-devirt/`)

Isolated `dyn Subscriber` dispatch (no tracing infrastructure overhead):

| Benchmark | devirt | plain vtable | Speedup |
|-----------|--------|-------------|---------|
| `enabled()` single call | 0.94 ns | 1.60 ns | **1.7×** |
| Span lifecycle (3 calls) | 1.45 ns | 4.65 ns | **3.2×** |
| Event pipeline (2 calls) | 1.42 ns | 2.81 ns | **2.0×** |
| Shuffled n=1000 span lifecycle | 1.54 µs | 4.89 µs | **3.2×** |

### Real-world tracing macros (`tracing/`)

Full `tracing::info!()` / `tracing::span!()` pipeline with devirt
patched into the actual `Subscriber` trait:

| Benchmark | baseline | devirt | Change |
|-----------|----------|--------|--------|
| `tracing::info!("hello")` | 11.70 ns | 11.92 ns | +1.2% (noise) |
| `tracing::info!(x=42)` | 12.18 ns | 12.24 ns | ~0% (noise) |
| `tracing::debug!()` filtered | 367.7 ps | 365.1 ps | -3.3% |
| span create+enter+exit | 31.21 ns | 32.81 ns | +5.1% |
| nested spans (2 deep) | 63.83 ns | 67.97 ns | +4.1% |
| span + 3 events | 49.88 ns | 51.01 ns | +5.2% |

### Interpretation

Devirt eliminates vtable dispatch overhead entirely for hot types (1.7-3.2×
in microbenchmarks). However, in real tracing usage, the vtable call is a
tiny fraction of total cost — the hot path is dominated by callsite
registration, field construction, and subscriber bookkeeping. The extra
vtable-pointer comparison in the devirt shim adds slight overhead that
isn't recouped by eliminating the indirect call.

**Where devirt shines:** tight loops dispatching cheap methods through
`dyn Trait` where the indirect call is the bottleneck (ECS updates,
AST walkers, codec dispatch).

**Where devirt doesn't help:** complex dispatch pipelines where the
method call overhead is dwarfed by surrounding bookkeeping (tracing,
logging frameworks).

## Running

### Microbenchmarks

```bash
cd benchmarks/tracing-devirt
cargo bench
```

### Real-world tracing

```bash
cd benchmarks/tracing

# Baseline (plain vtable dispatch)
cargo bench --bench devirt_event

# With devirt (vtable-pointer comparison dispatch)
cargo bench --bench devirt_event --features devirt-bench
```

Criterion automatically compares the second run against the first.

## Tracing subtree

Git subtree of `tokio-rs/tracing`. Update with:

```bash
git subtree pull --prefix=benchmarks/tracing https://github.com/tokio-rs/tracing.git main --squash
```
