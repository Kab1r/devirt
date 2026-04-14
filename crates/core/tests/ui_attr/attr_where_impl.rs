use std::fmt::Display;

struct Hot {
    val: f64,
}

#[devirt::devirt(Hot)]
pub trait Shape {
    fn describe(&self) -> String;
}

#[devirt::devirt]
impl Shape for Hot {
    fn describe(&self) -> String {
        format!("hot: {}", self.val)
    }
}

struct Named<T> {
    name: String,
    inner: T,
}

#[devirt::devirt]
impl<T> Shape for Named<T>
where
    T: Shape + Display,
{
    fn describe(&self) -> String {
        format!("{}: {}", self.name, self.inner)
    }
}

fn main() {}
