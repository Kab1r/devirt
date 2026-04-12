struct Hot {
    name: String,
}

struct Cold {
    name: String,
}

#[devirt::devirt(Hot)]
pub trait Named {
    fn name<'a>(&'a self) -> &'a str;
}

#[devirt::devirt]
impl Named for Hot {
    fn name<'a>(&'a self) -> &'a str { &self.name }
}

#[devirt::devirt]
impl Named for Cold {
    fn name<'a>(&'a self) -> &'a str { &self.name }
}

fn greet(n: &dyn Named) -> String {
    format!("Hello, {}!", n.name())
}

fn main() {
    let h = Hot { name: "world".into() };
    let c = Cold { name: "rust".into() };
    assert_eq!(greet(&h), "Hello, world!");
    assert_eq!(greet(&c), "Hello, rust!");
}
