struct Foo;

devirt::r#trait! {
    pub TwoMethods [Foo] {
        fn first(&self) -> i32;
        fn second(&self) -> i32;
    }
}

devirt::r#impl!(TwoMethods for Foo [hot] {
    fn first(&self) -> i32 { 1 }
});

fn main() {}
