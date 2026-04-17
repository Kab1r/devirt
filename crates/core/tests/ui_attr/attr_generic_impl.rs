struct Hot {
    val: f64,
}

#[devirt::devirt(Hot)]
pub trait Shape {
    fn area(&self) -> f64;
}

#[devirt::devirt]
impl Shape for Hot {
    fn area(&self) -> f64 {
        self.val
    }
}

// Generic impl — cold type, falls back to vtable
struct Scaled<T> {
    inner: T,
    factor: f64,
}

#[devirt::devirt]
impl<T: Shape> Shape for Scaled<T> {
    fn area(&self) -> f64 {
        // Coerce to &dyn Shape for devirt dispatch. The rewriter only
        // rewrites `self.method()` calls, not `self.inner.method()`.
        self.factor * (&self.inner as &dyn Shape).area()
    }
}

fn total(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

fn main() {
    let shapes: Vec<Box<dyn Shape>> = vec![
        Box::new(Hot { val: 10.0 }),
        Box::new(Scaled { inner: Hot { val: 5.0 }, factor: 3.0 }),
    ];
    assert_eq!(total(&shapes), 25.0); // 10 + 5*3
}
