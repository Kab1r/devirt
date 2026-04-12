//! Kani bounded model checking harnesses for devirt dispatch.
//!
//! Verifies no-panic and dispatch equivalence for the concrete macro
//! expansions at N=1, N=2, and N=3 hot types.
//!
//! Each trait level uses distinct types to avoid method-name collisions
//! across the generated `__TraitNImpl` hidden traits.
#![cfg(kani)]
#![allow(missing_docs)]
// The `mod vt` section below uses `unsafe { &*(raw[0] as *const Hot) }`
// to round-trip the data half of a fat pointer. The crate-wide lint is
// `deny(unsafe_code)` and this file's harnesses are the only unsafe
// use site — allow it here only.
#![allow(unsafe_code)]

// ── N=1: base case — one witness check, then fallback ────────────────────

mod n1 {
    struct Hot {
        val: u64,
    }

    struct Cold {
        val: u64,
    }

    devirt::__devirt_define! {
        @trait []
        pub Trait1 [Hot] {
            fn compute(&self, x: u64) -> u64;
            fn notify(&self, x: u64);
            fn transform(&mut self, x: u64) -> u64;
            fn reset(&mut self, x: u64);
        }
    }

    devirt::__devirt_define! { @impl [] Trait1 for Hot {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_add(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x; }
    }}

    devirt::__devirt_define! { @impl [] Trait1 for Cold {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_sub(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
    }}

    #[kani::proof]
    fn t1_hot_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = Hot { val };
        let s: &dyn Trait1 = &obj;
        assert!(s.compute(x) == val.wrapping_add(x));
    }

    #[kani::proof]
    fn t1_hot_notify_no_panic() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = Hot { val };
        let s: &dyn Trait1 = &obj;
        s.notify(x);
    }

    #[kani::proof]
    fn t1_hot_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = Hot { val };
        let r = (&mut obj as &mut dyn Trait1).transform(x);
        assert!(r == val.wrapping_add(x));
        assert!(obj.val == val.wrapping_add(x));
    }

    #[kani::proof]
    fn t1_hot_reset_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = Hot { val };
        (&mut obj as &mut dyn Trait1).reset(x);
        assert!(obj.val == x);
    }

    #[kani::proof]
    fn t1_cold_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = Cold { val };
        let s: &dyn Trait1 = &obj;
        assert!(s.compute(x) == val.wrapping_sub(x));
    }

    #[kani::proof]
    fn t1_cold_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = Cold { val };
        let r = (&mut obj as &mut dyn Trait1).transform(x);
        assert!(r == val.wrapping_sub(x));
        assert!(obj.val == val.wrapping_sub(x));
    }
}

// ── N=2: one recursive step ──────────────────────────────────────────────

mod n2 {
    struct HotA {
        val: u64,
    }

    struct HotB {
        val: u64,
    }

    struct Cold {
        val: u64,
    }

    devirt::__devirt_define! {
        @trait []
        pub Trait2 [HotA, HotB] {
            fn compute(&self, x: u64) -> u64;
            fn notify(&self, x: u64);
            fn transform(&mut self, x: u64) -> u64;
            fn reset(&mut self, x: u64);
        }
    }

    devirt::__devirt_define! { @impl [] Trait2 for HotA {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_add(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x; }
    }}

    devirt::__devirt_define! { @impl [] Trait2 for HotB {
        fn compute(&self, x: u64) -> u64 { self.val | x }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val |= x; self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
    }}

    devirt::__devirt_define! { @impl [] Trait2 for Cold {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_sub(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(2); }
    }}

    #[kani::proof]
    fn t2_hot_a_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = HotA { val };
        let s: &dyn Trait2 = &obj;
        assert!(s.compute(x) == val.wrapping_add(x));
    }

    #[kani::proof]
    fn t2_hot_b_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = HotB { val };
        let s: &dyn Trait2 = &obj;
        assert!(s.compute(x) == (val | x));
    }

    #[kani::proof]
    fn t2_cold_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = Cold { val };
        let s: &dyn Trait2 = &obj;
        assert!(s.compute(x) == val.wrapping_sub(x));
    }

    #[kani::proof]
    fn t2_hot_a_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotA { val };
        let r = (&mut obj as &mut dyn Trait2).transform(x);
        assert!(r == val.wrapping_add(x));
        assert!(obj.val == val.wrapping_add(x));
    }

    #[kani::proof]
    fn t2_hot_b_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotB { val };
        let r = (&mut obj as &mut dyn Trait2).transform(x);
        assert!(r == (val | x));
        assert!(obj.val == (val | x));
    }

    #[kani::proof]
    fn t2_cold_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = Cold { val };
        let r = (&mut obj as &mut dyn Trait2).transform(x);
        assert!(r == val.wrapping_sub(x));
        assert!(obj.val == val.wrapping_sub(x));
    }

    #[kani::proof]
    fn t2_hot_a_reset_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotA { val };
        (&mut obj as &mut dyn Trait2).reset(x);
        assert!(obj.val == x);
    }

    #[kani::proof]
    fn t2_hot_b_reset_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotB { val };
        (&mut obj as &mut dyn Trait2).reset(x);
        assert!(obj.val == x.wrapping_add(1));
    }

    #[kani::proof]
    fn t2_cold_reset_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = Cold { val };
        (&mut obj as &mut dyn Trait2).reset(x);
        assert!(obj.val == x.wrapping_add(2));
    }
}

// ── N=3: two recursive steps ─────────────────────────────────────────────

mod n3 {
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

    devirt::__devirt_define! {
        @trait []
        pub Trait3 [HotA, HotB, HotC] {
            fn compute(&self, x: u64) -> u64;
            fn notify(&self, x: u64);
            fn transform(&mut self, x: u64) -> u64;
            fn reset(&mut self, x: u64);
        }
    }

    devirt::__devirt_define! { @impl [] Trait3 for HotA {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_add(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x; }
    }}

    devirt::__devirt_define! { @impl [] Trait3 for HotB {
        fn compute(&self, x: u64) -> u64 { self.val | x }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val |= x; self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
    }}

    devirt::__devirt_define! { @impl [] Trait3 for HotC {
        fn compute(&self, x: u64) -> u64 { self.val ^ x }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val ^= x; self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(2); }
    }}

    devirt::__devirt_define! { @impl [] Trait3 for Cold {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_sub(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(3); }
    }}

    #[kani::proof]
    fn t3_hot_a_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = HotA { val };
        let s: &dyn Trait3 = &obj;
        assert!(s.compute(x) == val.wrapping_add(x));
    }

    #[kani::proof]
    fn t3_hot_b_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = HotB { val };
        let s: &dyn Trait3 = &obj;
        assert!(s.compute(x) == (val | x));
    }

    #[kani::proof]
    fn t3_hot_c_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = HotC { val };
        let s: &dyn Trait3 = &obj;
        assert!(s.compute(x) == (val ^ x));
    }

    #[kani::proof]
    fn t3_cold_compute_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let obj = Cold { val };
        let s: &dyn Trait3 = &obj;
        assert!(s.compute(x) == val.wrapping_sub(x));
    }

    #[kani::proof]
    fn t3_hot_a_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotA { val };
        let r = (&mut obj as &mut dyn Trait3).transform(x);
        assert!(r == val.wrapping_add(x));
        assert!(obj.val == val.wrapping_add(x));
    }

    #[kani::proof]
    fn t3_hot_b_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotB { val };
        let r = (&mut obj as &mut dyn Trait3).transform(x);
        assert!(r == (val | x));
        assert!(obj.val == (val | x));
    }

    #[kani::proof]
    fn t3_hot_c_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotC { val };
        let r = (&mut obj as &mut dyn Trait3).transform(x);
        assert!(r == (val ^ x));
        assert!(obj.val == (val ^ x));
    }

    #[kani::proof]
    fn t3_cold_transform_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = Cold { val };
        let r = (&mut obj as &mut dyn Trait3).transform(x);
        assert!(r == val.wrapping_sub(x));
        assert!(obj.val == val.wrapping_sub(x));
    }

    #[kani::proof]
    fn t3_hot_c_reset_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = HotC { val };
        (&mut obj as &mut dyn Trait3).reset(x);
        assert!(obj.val == x.wrapping_add(2));
    }

    #[kani::proof]
    fn t3_cold_reset_equiv() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let mut obj = Cold { val };
        (&mut obj as &mut dyn Trait3).reset(x);
        assert!(obj.val == x.wrapping_add(3));
    }
}

// ── vtable primitive soundness ───────────────────────────────────────────
//
// Harnesses that directly verify the unsafe vtable-comparison primitives
// emitted by the macro: fat-pointer extraction, vtable-identity
// determinism and distinctness, and round-trip soundness of the data
// pointer reinterpretation.

mod vt {
    struct Hot {
        val: u64,
    }
    struct Cold {
        val: u64,
    }

    devirt::__devirt_define! {
        @trait []
        pub TraitVt [Hot] {
            fn compute(&self, x: u64) -> u64;
        }
    }

    devirt::__devirt_define! { @impl [] TraitVt for Hot {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_add(x) }
    }}

    devirt::__devirt_define! { @impl [] TraitVt for Cold {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_sub(x) }
    }}

    /// The vtable extracted from a concrete `&dyn TraitVt` must match
    /// the vtable the macro computes via `__devirt_vtable_for::<T>()`.
    /// A mismatch would mean the hot-path comparison always misses,
    /// silently degrading to vtable dispatch.
    #[kani::proof]
    fn vt_hot_matches_vtable_for() {
        let val: u64 = kani::any();
        let hot = Hot { val };
        let dyn_ref: &dyn TraitVt = &hot;
        let raw = <dyn TraitVt>::__devirt_raw_parts(dyn_ref);
        assert!(raw[1] == <dyn TraitVt>::__devirt_vtable_for::<Hot>());
    }

    /// A cold type's vtable must NOT match any hot type's vtable.
    /// Equivalently: distinct types produce distinct vtables. A
    /// collision would break the type-identity premise behind the
    /// unsafe `*const Cold → *const Hot` reinterpretation.
    #[kani::proof]
    fn vt_cold_differs_from_hot() {
        let val: u64 = kani::any();
        let cold = Cold { val };
        let dyn_ref: &dyn TraitVt = &cold;
        let raw = <dyn TraitVt>::__devirt_raw_parts(dyn_ref);
        assert!(raw[1] != <dyn TraitVt>::__devirt_vtable_for::<Hot>());
    }

    /// `__devirt_vtable_for::<T>()` must be deterministic: two
    /// successive calls return the same address within a single run,
    /// otherwise the compile-time `<Hot>::__devirt_vtable_for()` we
    /// compare against at each call site could drift relative to the
    /// runtime value extracted from the fat pointer.
    #[kani::proof]
    fn vt_for_is_deterministic() {
        let a = <dyn TraitVt>::__devirt_vtable_for::<Hot>();
        let b = <dyn TraitVt>::__devirt_vtable_for::<Hot>();
        assert!(a == b);
    }

    /// Reinterpreting the data half of a matched vtable as
    /// `*const Hot` and reading it back yields the original value.
    /// This is the core round-trip invariant that makes the
    /// reinterpretation sound.
    #[kani::proof]
    fn vt_data_half_roundtrips() {
        let val: u64 = kani::any();
        let hot = Hot { val };
        let dyn_ref: &dyn TraitVt = &hot;
        let raw = <dyn TraitVt>::__devirt_raw_parts(dyn_ref);
        // Vtable must match, so the cast is sound.
        assert!(raw[1] == <dyn TraitVt>::__devirt_vtable_for::<Hot>());
        // SAFETY: vtable match → `raw[0]` is a valid `*const Hot`,
        // with the same provenance as `dyn_ref`.
        let round_tripped: &Hot = unsafe { &*(raw[0] as *const Hot) };
        assert!(round_tripped.val == val);
    }

    /// End-to-end equivalence under vtable-cmp dispatch: calling a
    /// method through `&dyn TraitVt` on a hot type produces the same
    /// result as calling the concrete method directly. This is
    /// redundant with `n1::t1_hot_compute_equiv` but is phrased in
    /// terms of the new primitives to double-check that the
    /// vtable-comparison rewrite did not change observable behavior.
    #[kani::proof]
    fn vt_dispatch_equivalence() {
        let val: u64 = kani::any();
        let x: u64 = kani::any();
        let hot = Hot { val };
        let dyn_ref: &dyn TraitVt = &hot;
        assert!(dyn_ref.compute(x) == val.wrapping_add(x));
    }
}
