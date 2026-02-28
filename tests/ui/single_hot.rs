struct Foo {
    val: f64,
}

devirt::r#trait! {
    pub SingleHot [Foo] {
        fn get(&self) -> f64;
    }
}

devirt::r#impl!(SingleHot for Foo [hot] {
    fn get(&self) -> f64 { self.val }
});

fn main() {
    let f: Box<dyn SingleHot> = Box::new(Foo { val: 1.0 });
    assert_eq!(f.get(), 1.0);
}
