struct Foo {
    val: f64,
}

devirt::__devirt_define! {
    @trait []
    pub SingleHot [Foo] {
        fn get(&self) -> f64;
    }
}

devirt::__devirt_define! { @impl [] SingleHot for Foo {
    fn get(&self) -> f64 { self.val }
}}

fn main() {
    let f: Box<dyn SingleHot> = Box::new(Foo { val: 1.0 });
    assert_eq!(f.get(), 1.0);
}
