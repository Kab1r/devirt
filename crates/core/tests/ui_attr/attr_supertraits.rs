use std::fmt::Debug;

#[derive(Debug)]
struct Hot {
    val: u64,
}

#[derive(Debug)]
struct Cold {
    val: u64,
}

#[devirt::devirt(Hot)]
pub trait Inspectable: Debug {
    fn value(&self) -> u64;
}

#[devirt::devirt]
impl Inspectable for Hot {
    fn value(&self) -> u64 { self.val }
}

#[devirt::devirt]
impl Inspectable for Cold {
    fn value(&self) -> u64 { self.val + 1 }
}

fn inspect(i: &dyn Inspectable) -> String {
    format!("{:?} = {}", i, i.value())
}

fn main() {
    let h = Hot { val: 42 };
    let c = Cold { val: 42 };
    let s = inspect(&h);
    assert!(s.contains("42"), "expected '42' in '{s}'");
    let s = inspect(&c);
    assert!(s.contains("43"), "expected '43' in '{s}'");
}
