struct Hot {
    val: f64,
}

#[devirt::devirt(Hot)]
pub trait Shape {
    fn area(&self) -> f64;
    fn is_large(&self) -> bool {
        self.area() > 100.0
    }
}

#[devirt::devirt]
impl Shape for Hot {
    fn area(&self) -> f64 {
        self.val
    }
    fn is_large(&self) -> bool {
        false // override default
    }
}

fn main() {
    let h = Hot { val: 200.0 };
    let d: &dyn Shape = &h;
    assert!(!d.is_large()); // overridden to always false
}
