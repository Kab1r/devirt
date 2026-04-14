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
pub trait Inspectable
where
    Self: Debug,
{
    fn value(&self) -> u64;
    fn inspect(&self) -> String {
        format!("{:?} = {}", self, self.value())
    }
}

#[devirt::devirt]
impl Inspectable for Hot {
    fn value(&self) -> u64 {
        self.val
    }
}

#[devirt::devirt]
impl Inspectable for Cold {
    fn value(&self) -> u64 {
        self.val + 1
    }
}

fn check(i: &dyn Inspectable) -> String {
    i.inspect()
}

fn main() {
    let h = Hot { val: 42 };
    assert!(check(&h).contains("42"));

    let c = Cold { val: 10 };
    assert!(check(&c).contains("11"));
}
