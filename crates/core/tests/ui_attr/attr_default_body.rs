use std::fmt::Write;

struct Hot {
    val: f64,
}

struct Cold {
    val: f64,
}

#[devirt::devirt(Hot)]
pub trait Shape {
    fn area(&self) -> f64;
    fn is_large(&self) -> bool {
        self.area() > 100.0
    }
    fn describe(&self) -> String {
        let mut s = String::new();
        if self.is_large() {
            write!(s, "large (area={})", self.area()).ok();
        } else {
            write!(s, "small (area={})", self.area()).ok();
        }
        s
    }
}

#[devirt::devirt]
impl Shape for Hot {
    fn area(&self) -> f64 {
        self.val
    }
}

#[devirt::devirt]
impl Shape for Cold {
    fn area(&self) -> f64 {
        self.val + 1.0
    }
}

fn main() {
    // Hot type, small
    let h = Hot { val: 50.0 };
    let d: &dyn Shape = &h;
    assert!(!d.is_large());
    assert!(d.describe().contains("small"));

    // Hot type, large
    let big = Hot { val: 200.0 };
    let d2: &dyn Shape = &big;
    assert!(d2.is_large());
    assert!(d2.describe().contains("large"));

    // Cold type
    let c = Cold { val: 150.0 };
    let d3: &dyn Shape = &c;
    assert!(d3.is_large());
    assert!(d3.describe().contains("large"));
}
