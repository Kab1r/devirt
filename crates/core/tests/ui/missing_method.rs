struct Foo;

devirt::__devirt_define! {
    @trait []
    pub TwoMethods [Foo] {
        fn first(&self) -> i32;
        fn second(&self) -> i32;
    }
}

devirt::__devirt_define! { @impl [] TwoMethods for Foo {
    fn first(&self) -> i32 { 1 }
}}

fn main() {}
