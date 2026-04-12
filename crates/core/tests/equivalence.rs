#![allow(missing_docs, clippy::tests_outside_test_module)]

struct Hot {
    val: u64,
}

struct Cold {
    val: u64,
}

#[cfg(not(feature = "macros"))]
mod decl {
    use super::*;

    devirt::devirt! {
        pub trait T [Hot] {
            fn get(&self) -> u64;
            fn notify(&self, x: u64);
            fn transform(&mut self, x: u64) -> u64;
            fn reset(&mut self, x: u64);
        }
    }

    devirt::devirt! {
        impl T for Hot {
            fn get(&self) -> u64 { self.val }
            fn notify(&self, _x: u64) { }
            fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
            fn reset(&mut self, x: u64) { self.val = x; }
        }
    }

    devirt::devirt! {
        impl T for Cold {
            fn get(&self) -> u64 { self.val + 1 }
            fn notify(&self, _x: u64) { }
            fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
            fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
        }
    }
}

#[cfg(feature = "macros")]
mod attr {
    use super::*;

    #[devirt::devirt(Hot)]
    pub trait T {
        fn get(&self) -> u64;
        fn notify(&self, x: u64);
        fn transform(&mut self, x: u64) -> u64;
        fn reset(&mut self, x: u64);
    }

    #[devirt::devirt]
    impl T for Hot {
        fn get(&self) -> u64 { self.val }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_add(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x; }
    }

    #[devirt::devirt]
    impl T for Cold {
        fn get(&self) -> u64 { self.val + 1 }
        fn notify(&self, _x: u64) { }
        fn transform(&mut self, x: u64) -> u64 { self.val = self.val.wrapping_sub(x); self.val }
        fn reset(&mut self, x: u64) { self.val = x.wrapping_add(1); }
    }
}

#[cfg(not(feature = "macros"))]
#[test]
fn decl_dispatch() {
    // @dispatch_ref: &self, non-void
    let h = Hot { val: 42 };
    let c = Cold { val: 42 };
    assert_eq!((&h as &dyn decl::T).get(), 42);
    assert_eq!((&c as &dyn decl::T).get(), 43);

    // @dispatch_void: &self, void
    (&h as &dyn decl::T).notify(1);
    (&c as &dyn decl::T).notify(1);

    // @dispatch_mut: &mut self, non-void
    let mut h = Hot { val: 10 };
    let mut c = Cold { val: 10 };
    assert_eq!((&mut h as &mut dyn decl::T).transform(5), 15);
    assert_eq!((&mut c as &mut dyn decl::T).transform(3), 7);

    // @dispatch_mut_void: &mut self, void
    (&mut h as &mut dyn decl::T).reset(99);
    (&mut c as &mut dyn decl::T).reset(99);
    assert_eq!(h.val, 99);
    assert_eq!(c.val, 100);
}

// ── Auto-trait dispatch: dyn Trait + Send / Sync / Send + Sync ──────────────

#[cfg(feature = "macros")]
#[test]
fn attr_auto_trait_dispatch() {
    // Verify that dispatch through &(dyn T + Send), &(dyn T + Sync),
    // and &(dyn T + Send + Sync) produces the same results as &dyn T
    // for both hot and cold types.

    // --- &self, non-void (hot) ---
    let h = Hot { val: 42 };
    let base = (&h as &dyn attr::T).get();
    assert_eq!((&h as &(dyn attr::T + Send)).get(), base);
    assert_eq!((&h as &(dyn attr::T + Sync)).get(), base);
    assert_eq!((&h as &(dyn attr::T + Send + Sync)).get(), base);

    // --- &self, non-void (cold) ---
    let c = Cold { val: 42 };
    let base = (&c as &dyn attr::T).get();
    assert_eq!((&c as &(dyn attr::T + Send)).get(), base);
    assert_eq!((&c as &(dyn attr::T + Sync)).get(), base);
    assert_eq!((&c as &(dyn attr::T + Send + Sync)).get(), base);

    // --- &self, void ---
    (&h as &(dyn attr::T + Send)).notify(1);
    (&h as &(dyn attr::T + Sync)).notify(1);
    (&h as &(dyn attr::T + Send + Sync)).notify(1);

    // --- &mut self, non-void (hot) ---
    let mut h1 = Hot { val: 10 };
    let mut h2 = Hot { val: 10 };
    let expected = (&mut h1 as &mut dyn attr::T).transform(5);
    assert_eq!(
        (&mut h2 as &mut (dyn attr::T + Send)).transform(5),
        expected,
    );

    // --- &mut self, void (hot) ---
    let mut h3 = Hot { val: 10 };
    let mut h4 = Hot { val: 10 };
    (&mut h3 as &mut dyn attr::T).reset(99);
    (&mut h4 as &mut (dyn attr::T + Send)).reset(99);
    assert_eq!(h3.val, h4.val);

    // --- Box<dyn T + Send> ---
    let boxed: Box<dyn attr::T + Send> = Box::new(Hot { val: 7 });
    assert_eq!(boxed.get(), 7);
}

// ── Default method bodies ──────────────────────────────────────────────────

#[cfg(feature = "macros")]
mod attr_defaults {
    pub struct DefHot {
        pub val: u64,
    }

    pub struct DefCold {
        pub val: u64,
    }

    /// Overrides the default `is_big`.
    pub struct DefOver {
        pub val: u64,
    }

    /// Relies on the `describe` default that uses `format!`.
    pub struct DefFmt {
        pub val: u64,
    }

    #[devirt::devirt(DefHot)]
    pub trait Defaulted {
        fn get(&self) -> u64;
        fn is_big(&self) -> bool {
            self.get() > 100
        }
        fn describe(&self) -> String {
            format!("val={}", self.get())
        }
    }

    #[devirt::devirt]
    impl Defaulted for DefHot {
        fn get(&self) -> u64 {
            self.val
        }
    }

    #[devirt::devirt]
    impl Defaulted for DefCold {
        fn get(&self) -> u64 {
            self.val + 1
        }
    }

    #[devirt::devirt]
    impl Defaulted for DefOver {
        fn get(&self) -> u64 {
            self.val
        }
        fn is_big(&self) -> bool {
            // Exercises sibling-call rewriting inside impl bodies:
            // self.get() must be rewritten to self.__spec_get().
            self.get() > 1000
        }
    }

    #[devirt::devirt]
    impl Defaulted for DefFmt {
        fn get(&self) -> u64 {
            self.val
        }
    }
}

#[cfg(feature = "macros")]
#[test]
fn attr_default_body_dispatch() {
    use attr_defaults::{DefCold, DefFmt, DefHot, DefOver, Defaulted};

    // Hot type via &dyn Trait
    let h = DefHot { val: 200 };
    assert!((&h as &dyn Defaulted).is_big());
    let h2 = DefHot { val: 50 };
    assert!(!(&h2 as &dyn Defaulted).is_big());

    // Cold type via &dyn Trait
    let c = DefCold { val: 200 };
    assert!((&c as &dyn Defaulted).is_big());

    // Via &(dyn Trait + Send)
    let h3 = DefHot { val: 200 };
    assert!((&h3 as &(dyn Defaulted + Send)).is_big());
    let c2 = DefCold { val: 50 };
    assert!(!(&c2 as &(dyn Defaulted + Send)).is_big());

    // Via &(dyn Trait + Send + Sync)
    let h4 = DefHot { val: 200 };
    assert!((&h4 as &(dyn Defaulted + Send + Sync)).is_big());

    // Overridden default: DefOver uses `self.get() > 1000` (tests
    // sibling-call rewriting in impl bodies).
    let o = DefOver { val: 200 };
    assert!(!(&o as &dyn Defaulted).is_big());
    assert!(!(&o as &(dyn Defaulted + Send)).is_big());
    let o2 = DefOver { val: 2000 };
    assert!((&o2 as &dyn Defaulted).is_big());

    // Default body with write! macro (exercises token-level rewriting)
    let f = DefFmt { val: 42 };
    assert_eq!((&f as &dyn Defaulted).describe(), "val=42");
    assert_eq!((&f as &(dyn Defaulted + Send)).describe(), "val=42");
    // Hot type's describe (also via default body)
    let h5 = DefHot { val: 7 };
    assert_eq!((&h5 as &dyn Defaulted).describe(), "val=7");
}

// ── Extended proc-macro tests: supertraits, method lifetimes, #[must_use] ──

#[cfg(feature = "macros")]
mod attr_extended {
    use core::fmt;

    #[derive(Debug)]
    pub struct ExtHot {
        pub val: u64,
        pub label: String,
    }

    #[derive(Debug)]
    pub struct ExtCold {
        pub val: u64,
        pub label: String,
    }

    #[devirt::devirt(ExtHot)]
    pub trait Inspectable: fmt::Debug {
        #[must_use]
        fn value(&self) -> u64;
        // Explicit lifetime to exercise method-lifetime support.
        fn name<'a>(&'a self) -> &'a str;
        fn set_val(&mut self, v: u64);
    }

    #[devirt::devirt]
    impl Inspectable for ExtHot {
        fn value(&self) -> u64 { self.val }
        fn name(&self) -> &str { &self.label }
        fn set_val(&mut self, v: u64) { self.val = v; }
    }

    #[devirt::devirt]
    impl Inspectable for ExtCold {
        fn value(&self) -> u64 { self.val + 1 }
        fn name(&self) -> &str { &self.label }
        fn set_val(&mut self, v: u64) { self.val = v + 1; }
    }
}

#[cfg(feature = "macros")]
#[test]
fn attr_extended_dispatch() {
    use attr_extended::{ExtCold, ExtHot, Inspectable};

    // Supertraits: dyn Inspectable implements Debug
    let h = ExtHot { val: 42, label: "hot".into() };
    let c = ExtCold { val: 42, label: "cold".into() };
    drop(format!("{:?}", &h as &dyn Inspectable));

    // #[must_use] + non-void &self
    assert_eq!((&h as &dyn Inspectable).value(), 42);
    assert_eq!((&c as &dyn Inspectable).value(), 43);

    // Method lifetimes
    assert_eq!((&h as &dyn Inspectable).name(), "hot");
    assert_eq!((&c as &dyn Inspectable).name(), "cold");

    // &mut self
    let mut h = ExtHot { val: 0, label: "hot".into() };
    let mut c = ExtCold { val: 0, label: "cold".into() };
    (&mut h as &mut dyn Inspectable).set_val(10);
    (&mut c as &mut dyn Inspectable).set_val(10);
    assert_eq!(h.val, 10);
    assert_eq!(c.val, 11);
}

#[cfg(feature = "macros")]
#[test]
fn attr_dispatch() {
    // @dispatch_ref: &self, non-void
    let h = Hot { val: 42 };
    let c = Cold { val: 42 };
    assert_eq!((&h as &dyn attr::T).get(), 42);
    assert_eq!((&c as &dyn attr::T).get(), 43);

    // @dispatch_void: &self, void
    (&h as &dyn attr::T).notify(1);
    (&c as &dyn attr::T).notify(1);

    // @dispatch_mut: &mut self, non-void
    let mut h = Hot { val: 10 };
    let mut c = Cold { val: 10 };
    assert_eq!((&mut h as &mut dyn attr::T).transform(5), 15);
    assert_eq!((&mut c as &mut dyn attr::T).transform(3), 7);

    // @dispatch_mut_void: &mut self, void
    (&mut h as &mut dyn attr::T).reset(99);
    (&mut c as &mut dyn attr::T).reset(99);
    assert_eq!(h.val, 99);
    assert_eq!(c.val, 100);
}
