#![allow(missing_docs, clippy::tests_outside_test_module)]

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/single_hot.rs");
    t.pass("tests/ui/multi_hot.rs");
    t.pass("tests/ui/all_arms.rs");
    t.pass("tests/ui/multi_arg.rs");
    t.pass("tests/ui/pub_trait.rs");
}
