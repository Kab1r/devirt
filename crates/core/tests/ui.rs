#![allow(missing_docs, clippy::tests_outside_test_module)]

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/single_hot.rs");
    t.pass("tests/ui/multi_hot.rs");
    t.pass("tests/ui/all_arms.rs");
    t.pass("tests/ui/multi_arg.rs");
    t.pass("tests/ui/pub_trait.rs");
    t.pass("tests/ui/unsafe_trait.rs");
    t.pass("tests/ui/method_attrs.rs");
    t.compile_fail("tests/ui/missing_method.rs");
    t.compile_fail("tests/ui/wrong_signature.rs");
}
