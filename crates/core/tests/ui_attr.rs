#![allow(missing_docs, clippy::tests_outside_test_module)]

#[test]
fn ui_attr() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui_attr/attr_single_hot.rs");
    t.pass("tests/ui_attr/attr_multi_hot.rs");
    t.pass("tests/ui_attr/attr_all_arms.rs");
    t.pass("tests/ui_attr/attr_unsafe_trait.rs");
    t.pass("tests/ui_attr/attr_method_attrs.rs");
    t.pass("tests/ui_attr/attr_unsafe_fn.rs");
    t.pass("tests/ui_attr/attr_method_lifetimes.rs");
    t.pass("tests/ui_attr/attr_supertraits.rs");
    t.pass("tests/ui_attr/attr_must_use.rs");
    t.pass("tests/ui_attr/attr_dyn_send.rs");
    t.pass("tests/ui_attr/attr_default_body.rs");
    t.pass("tests/ui_attr/attr_default_override.rs");
    t.pass("tests/ui_attr/attr_default_send.rs");
    t.pass("tests/ui_attr/attr_where_clause.rs");
    t.pass("tests/ui_attr/attr_generic_impl.rs");
    t.pass("tests/ui_attr/attr_where_impl.rs");
    t.compile_fail("tests/ui_attr/attr_must_use_unused.rs");
    t.compile_fail("tests/ui_attr/attr_missing_args.rs");
    t.compile_fail("tests/ui_attr/attr_unsafe_missing_on_impl.rs");
    t.compile_fail("tests/ui_attr/attr_args_on_impl.rs");
    t.compile_fail("tests/ui_attr/attr_on_struct.rs");
}
