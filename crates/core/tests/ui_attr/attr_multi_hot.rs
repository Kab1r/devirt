struct A;
struct B;
struct C;

#[devirt::devirt(A, B, C)]
pub trait MultiHot {
    fn id(&self) -> u8;
}

#[devirt::devirt]
impl MultiHot for A {
    fn id(&self) -> u8 { 1 }
}

#[devirt::devirt]
impl MultiHot for B {
    fn id(&self) -> u8 { 2 }
}

#[devirt::devirt]
impl MultiHot for C {
    fn id(&self) -> u8 { 3 }
}

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
