//! Kani bounded model checking harnesses for devirt dispatch.
//!
//! Verifies no-panic and dispatch equivalence for the concrete macro
//! expansions at N=1, N=2, and N=3 hot types.
//!
//! Each trait level uses distinct types to avoid method-name collisions
//! across the generated `__TraitNImpl` hidden traits.
#![cfg(kani)]
#![allow(missing_docs)]

// ── N=1: base case — one witness check, then fallback ────────────────────

mod n1 {
    struct Hot {
        val: u64,
    }

    struct Cold {
        val: u64,
    }

    devirt::r#trait! {
        pub Trait1 [Hot] {
            fn compute(&self, x: u64) -> u64;
            fn notify(&self, x: u64);
            fn transform(&mut self, x: u64) -> u64;
            fn reset(&mut self, x: u64);
        }
    }

    devirt::r#impl!(Trait1 for Hot [hot] {
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

    devirt::r#trait! {
        pub Trait2 [HotA, HotB] {
            fn compute(&self, x: u64) -> u64;
            fn notify(&self, x: u64);
            fn transform(&mut self, x: u64) -> u64;
            fn reset(&mut self, x: u64);
        }
    }

    devirt::r#impl!(Trait2 for HotA [hot] {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_add(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x; }
    });

    devirt::r#impl!(Trait2 for HotB [hot] {
        fn compute(&self, x: u64) -> u64 { self.val | x }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val |= x; self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
    });

    devirt::r#impl!(Trait2 for Cold {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_sub(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(2); }
    });

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

    devirt::r#trait! {
        pub Trait3 [HotA, HotB, HotC] {
            fn compute(&self, x: u64) -> u64;
            fn notify(&self, x: u64);
            fn transform(&mut self, x: u64) -> u64;
            fn reset(&mut self, x: u64);
        }
    }

    devirt::r#impl!(Trait3 for HotA [hot] {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_add(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x; }
    });

    devirt::r#impl!(Trait3 for HotB [hot] {
        fn compute(&self, x: u64) -> u64 { self.val | x }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val |= x; self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
    });

    devirt::r#impl!(Trait3 for HotC [hot] {
        fn compute(&self, x: u64) -> u64 { self.val ^ x }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val ^= x; self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(2); }
    });

    devirt::r#impl!(Trait3 for Cold {
        fn compute(&self, x: u64) -> u64 { self.val.wrapping_sub(x) }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(3); }
    });

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
