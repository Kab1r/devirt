# Real-World Devirt Benchmarks

Benchmarks measuring `devirt`'s impact on dispatch patterns from real Rust projects.

## tantivy search engine — scorer dispatch (`tantivy-devirt/`)

Reproduces tantivy's per-document scoring loop. tantivy iterates over
matching documents calling `scorer.score()` (BM25: ~10 arithmetic ops)
and `scorer.advance()` through `&mut dyn Scorer` — mandatory dynamic
dispatch since the scorer type comes from the user's search query parsed
at runtime.

Reference: [tantivy `src/query/weight.rs`](https://github.com/quickwit-oss/tantivy/blob/main/src/query/weight.rs)

Note: tantivy already manually devirtualizes its hottest paths
(`TermWeight::for_each` calls the concrete `TermScorer` directly).
The `dyn Scorer` fallback runs for uncommon query types (phrase, regex,
fuzzy). This benchmark measures the speedup devirt would provide on that
fallback path, and demonstrates what tantivy achieves manually that devirt
could automate.

### Results (TermScorer with BM25 scoring)

| n documents | devirt | plain vtable | Speedup |
|-------------|--------|-------------|---------|
| 1,000 | 3.1 µs | 9.9 µs | **3.2×** |
| 10,000 | 27.8 µs | 98.9 µs | **3.6×** |
| 100,000 | 276 µs | 961 µs | **3.5×** |
| 1,000,000 | 3.54 ms | 10.3 ms | **2.9×** |

Shuffled 80/20 hot/cold (100 scorers × 1000 docs): 333 µs vs 821 µs — **2.5×**

### Running

```bash
cd benchmarks/tantivy-devirt
cargo bench
```
