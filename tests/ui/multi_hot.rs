struct A;
struct B;
struct C;

devirt::r#trait! {
    pub MultiHot [A, B, C] {
        fn id(&self) -> u8;
    }
}

devirt::r#impl!(MultiHot for A [hot] {
    fn id(&self) -> u8 { 1 }
});

devirt::r#impl!(MultiHot for B [hot] {
    fn id(&self) -> u8 { 2 }
});

devirt::r#impl!(MultiHot for C [hot] {
    fn id(&self) -> u8 { 3 }
});

fn main() {
    let items: Vec<Box<dyn MultiHot>> = vec![
        Box::new(A),
        Box::new(B),
        Box::new(C),
    ];
    assert_eq!(items[0].id(), 1);
    assert_eq!(items[1].id(), 2);
    assert_eq!(items[2].id(), 3);
}
