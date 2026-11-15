struct Circle {
    radius: f64,
}

#[devirt::devirt(Circle)]
trait Shape {
    fn radius(&self) -> f64;
    fn describe(&self) -> String;
}

#[devirt::devirt]
impl Shape for Circle {
    fn radius(&self) -> f64 {
        self.radius
    }
    fn describe(&self) -> String {
        format!("{} {}", self.radius, self.radius())
    }
}

fn main() {
    let c = Circle { radius: 1.5 };
    assert_eq!((&c as &dyn Shape).describe(), "1.5 1.5");
}
