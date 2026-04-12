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
}

fn check(s: &(dyn Shape + Send)) -> bool {
    s.is_large()
}

fn main() {
    let h = Hot { val: 200.0 };
    assert!(check(&h));

    let small = Hot { val: 50.0 };
    assert!(!check(&small));
}
