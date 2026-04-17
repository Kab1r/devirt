//! Benchmark measuring devirt's impact on the tracing `Subscriber` dispatch
//! pattern.
//!
//! The real `tracing_core::Subscriber` trait dispatches 7 required methods
//! through `&dyn Subscriber` on every span creation, event, enter, and exit.
//! This benchmark reproduces that exact dispatch shape — same method count,
//! same `&self` receiver, same argument/return patterns — to measure how much
//! devirt would speed up subscriber dispatch.
//!
//! Reference: `benchmarks/tracing/tracing-core/src/subscriber.rs`
//!
//! Method mapping to real Subscriber:
//!   enabled(level)       → Subscriber::enabled(&Metadata)     — filter check
//!   new_span(name)       → Subscriber::new_span(&Attributes)  — returns span ID
//!   record(span, value)  → Subscriber::record(&Id, &Record)   — no-op for most
//!   record_follows(a, b) → Subscriber::record_follows_from    — no-op for most
//!   event(level, msg)    → Subscriber::event(&Event)           — main logging
//!   enter(span)          → Subscriber::enter(&Id)              — span enter
//!   exit(span)           → Subscriber::exit(&Id)               — span exit

#![allow(dead_code)]

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

// ── Types ────────────────────────────────────────────────────────────────────

/// Simulates a subscriber that does real work: level filtering + lightweight
/// bookkeeping. Models `tracing_subscriber::fmt::FmtSubscriber`.
struct FmtSubscriber {
    min_level: u64,
    next_id: std::cell::Cell<u64>,
}

impl FmtSubscriber {
    fn new(min_level: u64) -> Self {
        Self {
            min_level,
            next_id: std::cell::Cell::new(1),
        }
    }
}

/// Simulates a no-op subscriber. Models `tracing_core::subscriber::NoSubscriber`.
struct NoSubscriber;

// ── Devirtualized trait (mirrors Subscriber's 7 required methods) ────────────

#[devirt::devirt(FmtSubscriber)]
trait Sub {
    fn enabled(&self, level: u64) -> bool;
    fn new_span(&self, name: u64) -> u64;
    fn record(&self, span: u64, value: u64);
    fn record_follows(&self, span: u64, follows: u64);
    fn event(&self, level: u64, msg: u64);
    fn enter(&self, span: u64);
    fn exit(&self, span: u64);
}

#[devirt::devirt]
impl Sub for FmtSubscriber {
    #[inline]
    fn enabled(&self, level: u64) -> bool {
        level >= self.min_level
    }
    #[inline]
    fn new_span(&self, name: u64) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        id ^ name
    }
    #[inline]
    fn record(&self, _span: u64, _value: u64) {}
    #[inline]
    fn record_follows(&self, _span: u64, _follows: u64) {}
    #[inline]
    fn event(&self, level: u64, msg: u64) {
        if level >= self.min_level {
            black_box(msg);
        }
    }
    #[inline]
    fn enter(&self, _span: u64) {}
    #[inline]
    fn exit(&self, _span: u64) {}
}

#[devirt::devirt]
impl Sub for NoSubscriber {
    #[inline]
    fn enabled(&self, _level: u64) -> bool {
        false
    }
    #[inline]
    fn new_span(&self, _name: u64) -> u64 {
        0xDEAD
    }
    #[inline]
    fn record(&self, _span: u64, _value: u64) {}
    #[inline]
    fn record_follows(&self, _span: u64, _follows: u64) {}
    #[inline]
    fn event(&self, _level: u64, _msg: u64) {}
    #[inline]
    fn enter(&self, _span: u64) {}
    #[inline]
    fn exit(&self, _span: u64) {}
}

// ── Plain trait (baseline, normal vtable dispatch) ───────────────────────────

trait PlainSub {
    fn enabled(&self, level: u64) -> bool;
    fn new_span(&self, name: u64) -> u64;
    fn record(&self, span: u64, value: u64);
    fn record_follows(&self, span: u64, follows: u64);
    fn event(&self, level: u64, msg: u64);
    fn enter(&self, span: u64);
    fn exit(&self, span: u64);
}

impl PlainSub for FmtSubscriber {
    #[inline]
    fn enabled(&self, level: u64) -> bool {
        level >= self.min_level
    }
    #[inline]
    fn new_span(&self, name: u64) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        id ^ name
    }
    #[inline]
    fn record(&self, _span: u64, _value: u64) {}
    #[inline]
    fn record_follows(&self, _span: u64, _follows: u64) {}
    #[inline]
    fn event(&self, level: u64, msg: u64) {
        if level >= self.min_level {
            black_box(msg);
        }
    }
    #[inline]
    fn enter(&self, _span: u64) {}
    #[inline]
    fn exit(&self, _span: u64) {}
}

impl PlainSub for NoSubscriber {
    #[inline]
    fn enabled(&self, _level: u64) -> bool {
        false
    }
    #[inline]
    fn new_span(&self, _name: u64) -> u64 {
        0xDEAD
    }
    #[inline]
    fn record(&self, _span: u64, _value: u64) {}
    #[inline]
    fn record_follows(&self, _span: u64, _follows: u64) {}
    #[inline]
    fn event(&self, _level: u64, _msg: u64) {}
    #[inline]
    fn enter(&self, _span: u64) {}
    #[inline]
    fn exit(&self, _span: u64) {}
}

// ── Benchmark: enabled() filter check ───────────────────────────────────────
// The most-called Subscriber method. Every `tracing::event!()` checks this.

fn bench_enabled(c: &mut Criterion) {
    let mut group = c.benchmark_group("subscriber_enabled");

    group.bench_function("devirt_hot", |b| {
        let sub: Box<dyn Sub> = Box::new(FmtSubscriber::new(2));
        let sub_ref: &dyn Sub = &*sub;
        b.iter(|| black_box(sub_ref).enabled(black_box(3)));
    });

    group.bench_function("plain_hot", |b| {
        let sub: Box<dyn PlainSub> = Box::new(FmtSubscriber::new(2));
        let sub_ref: &dyn PlainSub = &*sub;
        b.iter(|| black_box(sub_ref).enabled(black_box(3)));
    });

    group.bench_function("devirt_cold", |b| {
        let sub: Box<dyn Sub> = Box::new(NoSubscriber);
        let sub_ref: &dyn Sub = &*sub;
        b.iter(|| black_box(sub_ref).enabled(black_box(3)));
    });

    group.bench_function("plain_cold", |b| {
        let sub: Box<dyn PlainSub> = Box::new(NoSubscriber);
        let sub_ref: &dyn PlainSub = &*sub;
        b.iter(|| black_box(sub_ref).enabled(black_box(3)));
    });

    group.finish();
}

// ── Benchmark: span lifecycle (new_span + enter + exit) ─────────────────────
// Every span creation goes through these 3 calls in sequence.

fn bench_span_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("subscriber_span_lifecycle");

    group.bench_function("devirt_hot", |b| {
        let sub: Box<dyn Sub> = Box::new(FmtSubscriber::new(2));
        let sub_ref: &dyn Sub = &*sub;
        b.iter(|| {
            let s = black_box(sub_ref);
            let id = s.new_span(black_box(42));
            s.enter(black_box(id));
            s.exit(black_box(id));
        });
    });

    group.bench_function("plain_hot", |b| {
        let sub: Box<dyn PlainSub> = Box::new(FmtSubscriber::new(2));
        let sub_ref: &dyn PlainSub = &*sub;
        b.iter(|| {
            let s = black_box(sub_ref);
            let id = s.new_span(black_box(42));
            s.enter(black_box(id));
            s.exit(black_box(id));
        });
    });

    group.finish();
}

// ── Benchmark: event dispatch (enabled + event) ─────────────────────────────
// The hot path for logging: check enabled, then record the event.

fn bench_event(c: &mut Criterion) {
    let mut group = c.benchmark_group("subscriber_event");

    group.bench_function("devirt_hot", |b| {
        let sub: Box<dyn Sub> = Box::new(FmtSubscriber::new(2));
        let sub_ref: &dyn Sub = &*sub;
        b.iter(|| {
            let s = black_box(sub_ref);
            if s.enabled(black_box(3)) {
                s.event(black_box(3), black_box(0xCAFE));
            }
        });
    });

    group.bench_function("plain_hot", |b| {
        let sub: Box<dyn PlainSub> = Box::new(FmtSubscriber::new(2));
        let sub_ref: &dyn PlainSub = &*sub;
        b.iter(|| {
            let s = black_box(sub_ref);
            if s.enabled(black_box(3)) {
                s.event(black_box(3), black_box(0xCAFE));
            }
        });
    });

    group.finish();
}

// ── Benchmark: shuffled subscriber dispatch ─────────────────────────────────
// Simulates the scenario where multiple subscribers are layered or a collection
// of subscribers is iterated. Shuffled order prevents branch predictor learning.
// 90% FmtSubscriber (hot), 10% NoSubscriber (cold).

fn make_shuffled_devirt(n: usize) -> Vec<Box<dyn Sub>> {
    let mut v: Vec<Box<dyn Sub>> = Vec::with_capacity(n);
    for i in 0..n {
        let bucket = (i * 7 + 3) % 10;
        if bucket < 9 {
            v.push(Box::new(FmtSubscriber::new(2)));
        } else {
            v.push(Box::new(NoSubscriber));
        }
    }
    v
}

fn make_shuffled_plain(n: usize) -> Vec<Box<dyn PlainSub>> {
    let mut v: Vec<Box<dyn PlainSub>> = Vec::with_capacity(n);
    for i in 0..n {
        let bucket = (i * 7 + 3) % 10;
        if bucket < 9 {
            v.push(Box::new(FmtSubscriber::new(2)));
        } else {
            v.push(Box::new(NoSubscriber));
        }
    }
    v
}

fn bench_shuffled_enabled(c: &mut Criterion) {
    let mut group = c.benchmark_group("subscriber_shuffled_enabled");

    for &n in &[10_usize, 100, 1000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(format!("devirt_n{n}"), |b| {
            let subs = make_shuffled_devirt(n);
            b.iter(|| {
                let mut count = 0_u64;
                for s in &subs {
                    if black_box(s.as_ref()).enabled(black_box(3)) {
                        count += 1;
                    }
                }
                count
            });
        });

        group.bench_function(format!("plain_n{n}"), |b| {
            let subs = make_shuffled_plain(n);
            b.iter(|| {
                let mut count = 0_u64;
                for s in &subs {
                    if black_box(s.as_ref()).enabled(black_box(3)) {
                        count += 1;
                    }
                }
                count
            });
        });
    }

    group.finish();
}

// ── Benchmark: shuffled full event pipeline ─────────────────────────────────
// For each subscriber in a shuffled collection: enabled() → event().
// This is the full hot path for every tracing event in a layered subscriber.

fn bench_shuffled_event(c: &mut Criterion) {
    let mut group = c.benchmark_group("subscriber_shuffled_event");

    for &n in &[10_usize, 100, 1000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(format!("devirt_n{n}"), |b| {
            let subs = make_shuffled_devirt(n);
            b.iter(|| {
                for s in &subs {
                    let s = black_box(s.as_ref());
                    if s.enabled(black_box(3)) {
                        s.event(black_box(3), black_box(0xCAFE));
                    }
                }
            });
        });

        group.bench_function(format!("plain_n{n}"), |b| {
            let subs = make_shuffled_plain(n);
            b.iter(|| {
                for s in &subs {
                    let s = black_box(s.as_ref());
                    if s.enabled(black_box(3)) {
                        s.event(black_box(3), black_box(0xCAFE));
                    }
                }
            });
        });
    }

    group.finish();
}

// ── Benchmark: shuffled span lifecycle ──────────────────────────────────────
// Full span lifecycle through a shuffled subscriber collection.
// Each subscriber: new_span() → enter() → exit() = 3 dispatch calls each.

fn bench_shuffled_span(c: &mut Criterion) {
    let mut group = c.benchmark_group("subscriber_shuffled_span");

    for &n in &[10_usize, 100, 1000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(format!("devirt_n{n}"), |b| {
            let subs = make_shuffled_devirt(n);
            b.iter(|| {
                for s in &subs {
                    let s = black_box(s.as_ref());
                    let id = s.new_span(black_box(42));
                    s.enter(black_box(id));
                    s.exit(black_box(id));
                }
            });
        });

        group.bench_function(format!("plain_n{n}"), |b| {
            let subs = make_shuffled_plain(n);
            b.iter(|| {
                for s in &subs {
                    let s = black_box(s.as_ref());
                    let id = s.new_span(black_box(42));
                    s.enter(black_box(id));
                    s.exit(black_box(id));
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_enabled,
    bench_span_lifecycle,
    bench_event,
    bench_shuffled_enabled,
    bench_shuffled_event,
    bench_shuffled_span,
);
criterion_main!(benches);
