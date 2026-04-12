#![allow(missing_docs, clippy::tests_outside_test_module)]

#[test]
fn ui_attr() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui_attr/attr_single_hot.rs");
    t.pass("tests/ui_attr/attr_multi_hot.rs");
    t.pass("tests/ui_attr/attr_all_arms.rs");
    t.compile_fail("tests/ui_attr/attr_missing_args.rs");
    t.compile_fail("tests/ui_attr/attr_args_on_impl.rs");
    t.compile_fail("tests/ui_attr/attr_on_struct.rs");
}
