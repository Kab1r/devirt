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
