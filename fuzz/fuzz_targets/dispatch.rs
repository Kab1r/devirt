#![no_main]

use std::cell::Cell;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

// ── Types ───────────────────────────────────────────────────────────────────

struct HotA {
    val: f64,
    trace: Cell<f64>,
}

struct HotB {
    val: f64,
    trace: Cell<f64>,
}

struct Cold {
    val: f64,
    trace: Cell<f64>,
}

// ── Devirtualized trait ─────────────────────────────────────────────────────

devirt::r#trait! {
    pub Dispatch [HotA, HotB] {
        fn compute(&self, x: f64) -> f64;
        fn notify(&self, x: f64);
        fn transform(&mut self, x: f64) -> f64;
        fn reset(&mut self, x: f64);
        fn combine(&self, x: f64, y: f64) -> f64;
        fn val(&self) -> f64;
        fn trace_val(&self) -> f64;
    }
}

devirt::r#impl!(Dispatch for HotA [hot] {
    fn compute(&self, x: f64) -> f64 { self.val + x }
    fn notify(&self, x: f64) { self.trace.set(self.val + x); }
    fn transform(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 1.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val + x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
});

devirt::r#impl!(Dispatch for HotB [hot] {
    fn compute(&self, x: f64) -> f64 { self.val * x }
    fn notify(&self, x: f64) { self.trace.set(self.val * x); }
    fn transform(&mut self, x: f64) -> f64 { self.val *= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 2.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val * x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
});

devirt::r#impl!(Dispatch for Cold {
    fn compute(&self, x: f64) -> f64 { self.val - x }
    fn notify(&self, x: f64) { self.trace.set(self.val - x); }
    fn transform(&mut self, x: f64) -> f64 { self.val -= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 3.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val - x - y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
});

// ── Plain trait (baseline — normal vtable dispatch) ─────────────────────────

trait PlainDispatch {
    fn compute(&self, x: f64) -> f64;
    fn notify(&self, x: f64);
    fn transform(&mut self, x: f64) -> f64;
    fn reset(&mut self, x: f64);
    fn combine(&self, x: f64, y: f64) -> f64;
    fn val(&self) -> f64;
    fn trace_val(&self) -> f64;
}

impl PlainDispatch for HotA {
    fn compute(&self, x: f64) -> f64 { self.val + x }
    fn notify(&self, x: f64) { self.trace.set(self.val + x); }
    fn transform(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 1.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val + x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
}

impl PlainDispatch for HotB {
    fn compute(&self, x: f64) -> f64 { self.val * x }
    fn notify(&self, x: f64) { self.trace.set(self.val * x); }
    fn transform(&mut self, x: f64) -> f64 { self.val *= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 2.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val * x + y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
}

impl PlainDispatch for Cold {
    fn compute(&self, x: f64) -> f64 { self.val - x }
    fn notify(&self, x: f64) { self.trace.set(self.val - x); }
    fn transform(&mut self, x: f64) -> f64 { self.val -= x; self.val }
    fn reset(&mut self, x: f64) { self.val = x + 3.0; }
    fn combine(&self, x: f64, y: f64) -> f64 { self.val - x - y }
    fn val(&self) -> f64 { self.val }
    fn trace_val(&self) -> f64 { self.trace.get() }
}

// ── Fuzz input ──────────────────────────────────────────────────────────────

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    variant: u8,
    val: f64,
    x: f64,
    y: f64,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_devirt(variant: u8, val: f64) -> Box<dyn Dispatch> {
    match variant % 3 {
        0 => Box::new(HotA { val, trace: Cell::new(0.0) }),
        1 => Box::new(HotB { val, trace: Cell::new(0.0) }),
        _ => Box::new(Cold { val, trace: Cell::new(0.0) }),
    }
}

fn make_plain(variant: u8, val: f64) -> Box<dyn PlainDispatch> {
    match variant % 3 {
        0 => Box::new(HotA { val, trace: Cell::new(0.0) }),
        1 => Box::new(HotB { val, trace: Cell::new(0.0) }),
        _ => Box::new(Cold { val, trace: Cell::new(0.0) }),
    }
}

fn assert_bits_eq(a: f64, b: f64, msg: &str) {
    assert_eq!(
        a.to_bits(),
        b.to_bits(),
        "{msg}: devirt={a:?} plain={b:?}"
    );
}

// ── Fuzz target ─────────────────────────────────────────────────────────────

fuzz_target!(|input: FuzzInput| {
    // ── &self methods (non-mutating) ────────────────────────────────────
    let devirt = make_devirt(input.variant, input.val);
    let plain = make_plain(input.variant, input.val);

    // @dispatch_ref: &self, single arg -> f64
    assert_bits_eq(
        devirt.compute(input.x),
        plain.compute(input.x),
        "compute",
    );

    // @dispatch_ref: &self, multi-arg -> f64
    assert_bits_eq(
        devirt.combine(input.x, input.y),
        plain.combine(input.x, input.y),
        "combine",
    );

    // @dispatch_void: &self -> ()
    devirt.notify(input.x);
    plain.notify(input.x);
    assert_bits_eq(devirt.trace_val(), plain.trace_val(), "notify trace");

    // ── &mut self methods (mutating — fresh instances) ──────────────────
    let mut devirt_mut = make_devirt(input.variant, input.val);
    let mut plain_mut = make_plain(input.variant, input.val);

    // @dispatch_mut: &mut self -> f64
    assert_bits_eq(
        devirt_mut.transform(input.x),
        plain_mut.transform(input.x),
        "transform",
    );
    assert_bits_eq(devirt_mut.val(), plain_mut.val(), "state after transform");

    // @dispatch_mut_void: &mut self -> ()
    devirt_mut.reset(input.x);
    plain_mut.reset(input.x);
    assert_bits_eq(devirt_mut.val(), plain_mut.val(), "state after reset");
});
