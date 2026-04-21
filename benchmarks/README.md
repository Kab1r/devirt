# Real-World Devirt Benchmarks

Benchmarks measuring `devirt`'s impact on dispatch patterns from real Rust projects.

## Results Summary

### tantivy search engine — scorer dispatch (`tantivy-devirt/`)

tantivy's per-document scoring loop calls `scorer.score()` (BM25: ~10
arithmetic ops) and `scorer.advance()` through `&mut dyn Scorer` on every
matching document. This dispatch is **mandatory** — the scorer type comes
from the user's search query parsed at runtime.

| n documents | devirt | plain vtable | Speedup |
|-------------|--------|-------------|---------|
| 1,000 | 3.1 µs | 9.9 µs | **3.2×** |
| 10,000 | 27.8 µs | 98.9 µs | **3.6×** |
| 100,000 | 276 µs | 961 µs | **3.5×** |
| 1,000,000 | 3.54 ms | 10.3 ms | **2.9×** |

Shuffled 80/20 (100 scorers × 1000 docs): 333 µs vs 821 µs — **2.5×**

### tracing subscriber — event dispatch (`tracing/`, `tracing-devirt/`)

Isolated subscriber dispatch shows 1.7-3.2× speedup, but the full
`tracing::info!()` pipeline shows no benefit — dispatch overhead is
dwarfed by callsite registration, field construction, and bookkeeping.

| Benchmark | devirt | plain | Speedup |
|-----------|--------|-------|---------|
| Isolated `enabled()` call | 0.94 ns | 1.60 ns | **1.7×** |
| Isolated span lifecycle | 1.45 ns | 4.65 ns | **3.2×** |
| Full `tracing::info!()` | 11.92 ns | 11.70 ns | ~0% |
| Full span create+enter+exit | 32.81 ns | 31.21 ns | ~0% |

### When devirt helps vs. when it doesn't

**Devirt shines** when the `dyn Trait` method call is the bottleneck:
tight loops dispatching cheap methods (search scoring, per-element
processing, codec dispatch).

**Devirt doesn't help** when the vtable call is dwarfed by surrounding
work (tracing's callsite registration, heavy I/O, complex bookkeeping).

## Running

```bash
# tantivy scorer benchmarks
cd benchmarks/tantivy-devirt && cargo bench

# tracing microbenchmarks
cd benchmarks/tracing-devirt && cargo bench

# tracing real-world (compare two runs)
cd benchmarks/tracing
cargo bench --bench devirt_event                          # baseline
cargo bench --bench devirt_event --features devirt-bench  # with devirt
```

## Tracing subtree

Git subtree of `tokio-rs/tracing`. Update with:

```bash
git subtree pull --prefix=benchmarks/tracing https://github.com/tokio-rs/tracing.git main --squash
```
