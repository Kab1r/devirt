# Real-World Devirt Benchmarks

Benchmarks measuring `devirt`'s impact on dispatch patterns from popular Rust projects.

## tracing (`tracing-devirt/`)

Measures devirt's impact on the `tracing::Subscriber` dispatch pattern.
The real `tracing_core::Subscriber` trait dispatches 7 methods through
`&dyn Subscriber` on every span creation, event, enter, and exit.
This benchmark reproduces that dispatch shape.

Reference implementation: `tracing/tracing-core/src/subscriber.rs`

### Running

```bash
cd benchmarks/tracing-devirt
cargo bench
```

### Benchmark groups

- **`subscriber_enabled`** — single `enabled()` call (the most-called method)
- **`subscriber_span_lifecycle`** — `new_span()` + `enter()` + `exit()` sequence
- **`subscriber_event`** — `enabled()` + `event()` (logging hot path)
- **`subscriber_shuffled_enabled`** — shuffled 90/10 collection, `enabled()` only
- **`subscriber_shuffled_event`** — shuffled 90/10 collection, full event pipeline
- **`subscriber_shuffled_span`** — shuffled 90/10 collection, span lifecycle

## tracing subtree (`tracing/`)

Git subtree of `tokio-rs/tracing` (reference, unmodified). Update with:

```bash
git subtree pull --prefix=benchmarks/tracing https://github.com/tokio-rs/tracing.git main --squash
```
