//! Benchmark reproducing tantivy's per-document scorer dispatch.
//!
//! tantivy's `for_each_scorer_optimized()` iterates over matching documents,
//! calling `scorer.score()` and `scorer.advance()` through `&mut dyn Scorer`
//! on every document. This is mandatory dynamic dispatch — the scorer type
//! is determined by the user's query parsed at runtime.
//!
//! Reference: tantivy `src/query/weight.rs:9-18`
//!
//! The hot loop:
//!   while doc != TERMINATED {
//!       callback(doc, scorer.score());   // dyn dispatch — BM25: ~10 arith ops
//!       doc = scorer.advance();          // dyn dispatch — index + branch
//!   }
//!
//! TermScorer (BM25) dominates for single-term queries (~80% of real search).

#![allow(dead_code)]

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

const TERMINATED: u32 = u32::MAX;
const K1: f32 = 1.2;
const B: f32 = 0.75;

// ── Combined Scorer trait (mirrors tantivy's Scorer + DocSet) ───────────────
// All hot-path methods on one trait so devirt accelerates everything.

#[devirt::devirt(TermScorer)]
trait Scorer {
    fn score(&mut self) -> f32;
    fn advance(&mut self) -> u32;
    fn doc(&self) -> u32;
}

trait PlainScorer {
    fn score(&mut self) -> f32;
    fn advance(&mut self) -> u32;
    fn doc(&self) -> u32;
}

// ── TermScorer: BM25 scoring (the hot type, ~80% of real queries) ───────────

struct TermScorer {
    doc_ids: Vec<u32>,
    fieldnorm_ids: Vec<u8>,
    term_freqs: Vec<u32>,
    cursor: usize,
    weight: f32,
    average_fieldnorm: f32,
    idf_times_k1_plus_one: f32,
}

impl TermScorer {
    fn new(n: usize) -> Self {
        let mut doc_ids = Vec::with_capacity(n);
        let mut fieldnorm_ids = Vec::with_capacity(n);
        let mut term_freqs = Vec::with_capacity(n);
        for i in 0..n {
            doc_ids.push(i as u32 * 3);
            fieldnorm_ids.push(((i * 7 + 13) % 256) as u8);
            term_freqs.push(((i * 3 + 1) % 10 + 1) as u32);
        }
        Self {
            doc_ids,
            fieldnorm_ids,
            term_freqs,
            cursor: 0,
            weight: 2.5,
            average_fieldnorm: 128.0,
            idf_times_k1_plus_one: 3.2,
        }
    }

    #[inline]
    fn bm25_score(&self) -> f32 {
        let fieldnorm_id = self.fieldnorm_ids[self.cursor];
        let fieldnorm = FIELDNORM_CACHE[fieldnorm_id as usize];
        let term_freq = self.term_freqs[self.cursor];
        let tf = (term_freq as f32).min(255.0);
        let norm = K1 * (1.0 - B + B * fieldnorm as f32 / self.average_fieldnorm);
        self.weight * self.idf_times_k1_plus_one * tf / (tf + norm)
    }

    #[inline]
    fn do_advance(&mut self) -> u32 {
        self.cursor += 1;
        if self.cursor >= self.doc_ids.len() {
            return TERMINATED;
        }
        self.doc_ids[self.cursor]
    }

    #[inline]
    fn do_doc(&self) -> u32 {
        if self.cursor >= self.doc_ids.len() {
            TERMINATED
        } else {
            self.doc_ids[self.cursor]
        }
    }
}

static FIELDNORM_CACHE: [u32; 256] = {
    let mut cache = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        cache[i] = (i as u32).wrapping_mul(7).wrapping_add(1);
        i += 1;
    }
    cache
};

// ── ConstScorer: fixed score (boosted/filter queries) ───────────────────────

struct ConstScorer {
    doc_ids: Vec<u32>,
    cursor: usize,
    the_score: f32,
}

impl ConstScorer {
    fn new(n: usize, score: f32) -> Self {
        let doc_ids: Vec<u32> = (0..n as u32).map(|i| i * 5 + 1).collect();
        Self {
            doc_ids,
            cursor: 0,
            the_score: score,
        }
    }
}

// ── Devirt impls ────────────────────────────────────────────────────────────

#[devirt::devirt]
impl Scorer for TermScorer {
    #[inline]
    fn score(&mut self) -> f32 { self.bm25_score() }
    #[inline]
    fn advance(&mut self) -> u32 { self.do_advance() }
    #[inline]
    fn doc(&self) -> u32 { self.do_doc() }
}

#[devirt::devirt]
impl Scorer for ConstScorer {
    #[inline]
    fn score(&mut self) -> f32 { self.the_score }
    #[inline]
    fn advance(&mut self) -> u32 { self.do_advance() }
    #[inline]
    fn doc(&self) -> u32 { self.do_doc() }
}

impl ConstScorer {
    #[inline]
    fn do_advance(&mut self) -> u32 {
        self.cursor += 1;
        if self.cursor >= self.doc_ids.len() {
            return TERMINATED;
        }
        self.doc_ids[self.cursor]
    }
    #[inline]
    fn do_doc(&self) -> u32 {
        if self.cursor >= self.doc_ids.len() {
            TERMINATED
        } else {
            self.doc_ids[self.cursor]
        }
    }
}

// ── Plain impls ─────────────────────────────────────────────────────────────

impl PlainScorer for TermScorer {
    #[inline]
    fn score(&mut self) -> f32 { self.bm25_score() }
    #[inline]
    fn advance(&mut self) -> u32 { self.do_advance() }
    #[inline]
    fn doc(&self) -> u32 { self.do_doc() }
}

impl PlainScorer for ConstScorer {
    #[inline]
    fn score(&mut self) -> f32 { self.the_score }
    #[inline]
    fn advance(&mut self) -> u32 {
        self.cursor += 1;
        if self.cursor >= self.doc_ids.len() {
            return TERMINATED;
        }
        self.doc_ids[self.cursor]
    }
    #[inline]
    fn doc(&self) -> u32 {
        if self.cursor >= self.doc_ids.len() {
            TERMINATED
        } else {
            self.doc_ids[self.cursor]
        }
    }
}

// ── The hot loop: mirrors tantivy's for_each_scorer ─────────────────────────

#[inline(never)]
fn for_each_devirt(scorer: &mut dyn Scorer) -> (u32, f32) {
    let mut count = 0u32;
    let mut total_score = 0.0f32;
    let mut doc = scorer.doc();
    while doc != TERMINATED {
        total_score += scorer.score();
        count += 1;
        doc = scorer.advance();
    }
    black_box((count, total_score))
}

#[inline(never)]
fn for_each_plain(scorer: &mut dyn PlainScorer) -> (u32, f32) {
    let mut count = 0u32;
    let mut total_score = 0.0f32;
    let mut doc = scorer.doc();
    while doc != TERMINATED {
        total_score += scorer.score();
        count += 1;
        doc = scorer.advance();
    }
    black_box((count, total_score))
}

// ── Benchmarks ──────────────────────────────────────────────────────────────

fn bench_term_scorer(c: &mut Criterion) {
    let mut group = c.benchmark_group("term_scorer");

    for &n in &[1_000_usize, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(format!("devirt_n{n}"), |b| {
            b.iter_with_setup(
                || TermScorer::new(n),
                |mut scorer| {
                    for_each_devirt(&mut scorer)
                },
            );
        });

        group.bench_function(format!("plain_n{n}"), |b| {
            b.iter_with_setup(
                || TermScorer::new(n),
                |mut scorer| {
                    for_each_plain(&mut scorer)
                },
            );
        });
    }

    group.finish();
}

fn bench_const_scorer(c: &mut Criterion) {
    let mut group = c.benchmark_group("const_scorer");

    for &n in &[1_000_usize, 10_000, 100_000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(format!("devirt_n{n}"), |b| {
            b.iter_with_setup(
                || ConstScorer::new(n, 1.5),
                |mut scorer| {
                    for_each_devirt(&mut scorer)
                },
            );
        });

        group.bench_function(format!("plain_n{n}"), |b| {
            b.iter_with_setup(
                || ConstScorer::new(n, 1.5),
                |mut scorer| {
                    for_each_plain(&mut scorer)
                },
            );
        });
    }

    group.finish();
}

fn bench_shuffled_scorers(c: &mut Criterion) {
    let mut group = c.benchmark_group("shuffled_scorers");

    for &n in &[10_usize, 100] {
        let docs_per_scorer = 1000;
        group.throughput(Throughput::Elements((n * docs_per_scorer) as u64));

        group.bench_function(format!("devirt_{n}x{docs_per_scorer}"), |b| {
            b.iter_with_setup(
                || {
                    let mut v: Vec<Box<dyn Scorer>> = Vec::with_capacity(n);
                    for i in 0..n {
                        if (i * 7 + 3) % 10 < 8 {
                            v.push(Box::new(TermScorer::new(docs_per_scorer)));
                        } else {
                            v.push(Box::new(ConstScorer::new(docs_per_scorer, 1.0)));
                        }
                    }
                    v
                },
                |mut scorers| {
                    let mut total = 0.0f32;
                    for s in &mut scorers {
                        let (_, score) = for_each_devirt(s.as_mut());
                        total += score;
                    }
                    total
                },
            );
        });

        group.bench_function(format!("plain_{n}x{docs_per_scorer}"), |b| {
            b.iter_with_setup(
                || {
                    let mut v: Vec<Box<dyn PlainScorer>> = Vec::with_capacity(n);
                    for i in 0..n {
                        if (i * 7 + 3) % 10 < 8 {
                            v.push(Box::new(TermScorer::new(docs_per_scorer)));
                        } else {
                            v.push(Box::new(ConstScorer::new(docs_per_scorer, 1.0)));
                        }
                    }
                    v
                },
                |mut scorers| {
                    let mut total = 0.0f32;
                    for s in &mut scorers {
                        let (_, score) = for_each_plain(s.as_mut());
                        total += score;
                    }
                    total
                },
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_term_scorer, bench_const_scorer, bench_shuffled_scorers);
criterion_main!(benches);
