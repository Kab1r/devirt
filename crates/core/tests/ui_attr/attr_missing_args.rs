struct Foo;

#[devirt::devirt]
pub trait MissingArgs {
    fn get(&self) -> i32;
}

fn main() {}
