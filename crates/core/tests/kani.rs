//! Kani bounded model checking harnesses for devirt dispatch.
//!
//! Verifies no-panic and dispatch equivalence for the concrete macro
//! expansions at N=1, N=2, and N=3 hot types.
#![cfg(kani)]
#![allow(missing_docs)]

// ── Types ────────────────────────────────────────────────────────────────

struct HotA {
    val: u64,
}

struct HotB {
    val: u64,
}

struct HotC {
    val: u64,
}

struct Cold {
    val: u64,
}

// ── N=1: base case — one witness check, then fallback ────────────────────

devirt::r#trait! {
    pub Trait1 [HotA] {
        fn compute(&self, x: u64) -> u64;
        fn notify(&self, x: u64);
        fn transform(&mut self, x: u64) -> u64;
        fn reset(&mut self, x: u64);
    }
}

devirt::r#impl!(Trait1 for HotA [hot] {
    fn compute(&self, x: u64) -> u64 { self.val.wrapping_add(x) }
    fn notify(&self, _x: u64) { }
    fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
    fn reset(&mut self, x: u64) { self.val = x; }
});

devirt::r#impl!(Trait1 for Cold {
    fn compute(&self, x: u64) -> u64 { self.val.wrapping_sub(x) }
    fn notify(&self, _x: u64) { }
    fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
    fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
});

// ── N=1 harnesses ────────────────────────────────────────────────────────

#[kani::proof]
fn t1_hot_a_compute_equiv() {
    let val: u64 = kani::any();
    let x: u64 = kani::any();
    let obj = HotA { val };
    let direct = val.wrapping_add(x);
    let s: &dyn Trait1 = &obj;
    assert!(s.compute(x) == direct);
}

#[kani::proof]
fn t1_hot_a_notify_no_panic() {
    let val: u64 = kani::any();
    let x: u64 = kani::any();
    let obj = HotA { val };
    let s: &dyn Trait1 = &obj;
    s.notify(x);
}

#[kani::proof]
fn t1_hot_a_transform_equiv() {
    let val: u64 = kani::any();
    let x: u64 = kani::any();
    let mut obj = HotA { val };
    let mut obj2 = HotA { val };
    let r1 = (&mut obj as &mut dyn Trait1).transform(x);
    let r2 = obj2.val.wrapping_add(x);
    obj2.val = r2;
    assert!(r1 == r2);
    assert!(obj.val == obj2.val);
}

#[kani::proof]
fn t1_hot_a_reset_equiv() {
    let val: u64 = kani::any();
    let x: u64 = kani::any();
    let mut obj = HotA { val };
    (&mut obj as &mut dyn Trait1).reset(x);
    assert!(obj.val == x);
}

#[kani::proof]
fn t1_cold_compute_equiv() {
    let val: u64 = kani::any();
    let x: u64 = kani::any();
    let obj = Cold { val };
    let direct = val.wrapping_sub(x);
    let s: &dyn Trait1 = &obj;
    assert!(s.compute(x) == direct);
}

#[kani::proof]
fn t1_cold_transform_equiv() {
    let val: u64 = kani::any();
    let x: u64 = kani::any();
    let mut obj = Cold { val };
    let mut obj2 = Cold { val };
    let r1 = (&mut obj as &mut dyn Trait1).transform(x);
    let r2 = obj2.val.wrapping_sub(x);
    obj2.val = r2;
    assert!(r1 == r2);
    assert!(obj.val == obj2.val);
}
