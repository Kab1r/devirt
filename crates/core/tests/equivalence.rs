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
        }
    }

    devirt::devirt! {
        impl T for Hot {
            fn get(&self) -> u64 { self.val }
        }
    }

    devirt::devirt! {
        impl T for Cold {
            fn get(&self) -> u64 { self.val + 1 }
        }
    }
}

#[cfg(feature = "macros")]
mod attr {
    use super::*;

    #[devirt::devirt(Hot)]
    pub trait T {
        fn get(&self) -> u64;
    }

    #[devirt::devirt]
    impl T for Hot {
        fn get(&self) -> u64 { self.val }
    }

    #[devirt::devirt]
    impl T for Cold {
        fn get(&self) -> u64 { self.val + 1 }
    }
}

#[cfg(not(feature = "macros"))]
#[test]
fn decl_dispatch() {
    let h = Hot { val: 42 };
    let c = Cold { val: 42 };
    assert_eq!((&h as &dyn decl::T).get(), 42);
    assert_eq!((&c as &dyn decl::T).get(), 43);
}

#[cfg(feature = "macros")]
#[test]
fn attr_dispatch() {
    let h = Hot { val: 42 };
    let c = Cold { val: 42 };
    assert_eq!((&h as &dyn attr::T).get(), 42);
    assert_eq!((&c as &dyn attr::T).get(), 43);
}
