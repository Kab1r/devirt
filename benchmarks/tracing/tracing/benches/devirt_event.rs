//! Real-world benchmark: tracing event dispatch with devirt.
//!
//! Uses the actual `tracing::info!()` and `tracing::span!()` macros through
//! the full dispatch pipeline. Compare results with and without `devirt-bench`:
//!
//!   # baseline (plain vtable dispatch)
//!   cargo bench --bench devirt_event
//!
//!   # with devirt (vtable-pointer comparison dispatch)
//!   cargo bench --bench devirt_event --features devirt-bench

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tracing_core::bench_subscriber::BenchSubscriber;
use tracing_core::LevelFilter;

fn bench_event(c: &mut Criterion) {
    let mut group = c.benchmark_group("devirt_event");

    let sub = BenchSubscriber::new(LevelFilter::INFO);
    tracing::subscriber::with_default(sub, || {
        group.bench_function("info", |b| {
            b.iter(|| tracing::info!("hello world"))
        });

        group.bench_function("info_with_fields", |b| {
            b.iter(|| tracing::info!(x = black_box(42), y = black_box("test"), "event"))
        });

        group.bench_function("debug_filtered", |b| {
            b.iter(|| tracing::debug!("this is filtered out"))
        });
    });

    group.finish();
}

fn bench_span(c: &mut Criterion) {
    let mut group = c.benchmark_group("devirt_span");

    let sub = BenchSubscriber::new(LevelFilter::INFO);
    tracing::subscriber::with_default(sub, || {
        group.bench_function("create_enter_exit", |b| {
            b.iter(|| {
                let span = tracing::info_span!("my_span");
                let _guard = span.enter();
            })
        });

        group.bench_function("nested_spans", |b| {
            b.iter(|| {
                let outer = tracing::info_span!("outer");
                let _outer_guard = outer.enter();
                let inner = tracing::info_span!("inner");
                let _inner_guard = inner.enter();
            })
        });
    });

    group.finish();
}

fn bench_mixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("devirt_mixed");

    let sub = BenchSubscriber::new(LevelFilter::INFO);
    tracing::subscriber::with_default(sub, || {
        group.bench_function("span_with_events", |b| {
            b.iter(|| {
                let span = tracing::info_span!("request", id = black_box(42_u64));
                let _guard = span.enter();
                tracing::info!("processing");
                tracing::debug!("detail");
                tracing::info!("done");
            })
        });
    });

    group.finish();
}

criterion_group!(benches, bench_event, bench_span, bench_mixed);
criterion_main!(benches);
