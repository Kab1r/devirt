struct Foo;

devirt::__devirt_define! {
    @trait []
    pub ArgsOnImpl [Foo] {
        fn get(&self) -> i32;
    }
}

#[devirt::devirt(Foo)]
impl ArgsOnImpl for Foo {
    fn get(&self) -> i32 { 1 }
}

fn main() {}
