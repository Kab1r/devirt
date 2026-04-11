//! Demonstrates [`devirt`] with a `Shape` trait across hot and cold types.
//!
//! `Circle` and `Rect` are listed as hot types and get witness-method dispatch.
//! `Triangle` and `Hexagon` fall back to normal vtable dispatch.
//! All four are used identically through `dyn Shape`.
//!
//! Note: this example uses `std`; the `devirt` crate itself is `#![no_std]`.
#![expect(clippy::print_stdout, reason = "example intentionally prints output to demonstrate API usage")]

struct Circle { radius: f64 }
struct Rect { w: f64, h: f64 }
struct Triangle { a: f64, b: f64, c: f64 }
struct Hexagon { side: f64 }

// 1. Define trait — list hot types in brackets
devirt::r#trait! {
    /// Shapes with area, perimeter, and uniform scaling.
    pub Shape [Circle, Rect] {
        /// Returns the area of this shape.
        fn area(&self) -> f64;
        /// Returns the perimeter of this shape.
        fn perimeter(&self) -> f64;
        /// Scales this shape uniformly by `factor`.
        fn scale(&mut self, factor: f64);
        /// Scales and returns whether the shape is still within a 100×100 bounding box.
        fn try_scale(&mut self, factor: f64) -> bool;
        /// Prints a one-line description to stdout.
        fn describe(&self);
        /// Returns the human-readable name of this shape.
        fn name(&self) -> &str;
    }
}

// 2. Implement — normal-looking impl blocks; [hot] overrides witness methods
devirt::r#impl!(Shape for Circle [hot] {
    fn area(&self) -> f64 {
        core::f64::consts::PI * self.radius * self.radius
    }
    fn perimeter(&self) -> f64 {
        2.0 * core::f64::consts::PI * self.radius
    }
    fn scale(&mut self, factor: f64) {
        self.radius *= factor;
    }
    fn try_scale(&mut self, factor: f64) -> bool {
        self.radius *= factor;
        self.radius * 2.0 <= 100.0
    }
    fn describe(&self) {
        println!("circle with radius {:.2}", self.radius);
    }
    fn name(&self) -> &str { "circle" }
});

devirt::r#impl!(Shape for Rect [hot] {
    fn area(&self) -> f64 { self.w * self.h }
    fn perimeter(&self) -> f64 { 2.0 * (self.w + self.h) }
    fn scale(&mut self, factor: f64) {
        self.w *= factor;
        self.h *= factor;
    }
    fn try_scale(&mut self, factor: f64) -> bool {
        self.w *= factor;
        self.h *= factor;
        self.w <= 100.0 && self.h <= 100.0
    }
    fn describe(&self) {
        println!("rectangle {}×{:.2}", self.w, self.h);
    }
    fn name(&self) -> &str { "rectangle" }
});

devirt::r#impl!(Shape for Triangle {
    fn area(&self) -> f64 {
        let s = (self.a + self.b + self.c) / 2.0;
        (s * (s - self.a) * (s - self.b) * (s - self.c)).sqrt()
    }
    fn perimeter(&self) -> f64 { self.a + self.b + self.c }
    fn scale(&mut self, factor: f64) {
        self.a *= factor;
        self.b *= factor;
        self.c *= factor;
    }
    fn try_scale(&mut self, factor: f64) -> bool {
        self.a *= factor;
        self.b *= factor;
        self.c *= factor;
        let max = self.a.max(self.b).max(self.c);
        max <= 100.0
    }
    fn describe(&self) {
        println!("triangle with sides {:.2}, {:.2}, {:.2}", self.a, self.b, self.c);
    }
    fn name(&self) -> &str { "triangle" }
});

// Downstream type — not in the hot list, automatically uses vtable
devirt::r#impl!(Shape for Hexagon {
    fn area(&self) -> f64 { 1.5 * 3.0_f64.sqrt() * self.side * self.side }
    fn perimeter(&self) -> f64 { 6.0 * self.side }
    fn scale(&mut self, factor: f64) { self.side *= factor; }
    fn try_scale(&mut self, factor: f64) -> bool {
        self.side *= factor;
        self.side * 2.0 <= 100.0
    }
    fn describe(&self) {
        println!("regular hexagon with side {:.2}", self.side);
    }
    fn name(&self) -> &str { "hexagon" }
});

// 3. Use — completely normal dyn Trait. Nothing special.

fn print_shape(s: &dyn Shape) {
    println!("{:<10} area={:>8.2}  perim={:>8.2}",
        s.name(), s.area(), s.perimeter());
}

fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

fn main() {
    let mut shapes: Vec<Box<dyn Shape>> = vec![
        Box::new(Circle { radius: 5.0 }),                // → vtable-cmp hot
        Box::new(Rect { w: 3.0, h: 4.0 }),              // → vtable-cmp hot
        Box::new(Triangle { a: 3.0, b: 4.0, c: 5.0 }), // → vtable fallback
        Box::new(Hexagon { side: 2.0 }),                 // → vtable fallback
    ];

    println!("=== dyn Shape — devirtualization is invisible ===");
    for s in &shapes {
        print_shape(&**s);
    }
    println!("total area = {:.2}", total_area(&shapes));

    // void &self — exercises @dispatch_void path
    println!("\n=== describe (void &self) ===");
    for s in &shapes {
        s.describe();
    }

    for s in &mut shapes {
        s.scale(2.0);
    }

    println!("\nAfter 2x scale:");
    for s in &shapes {
        print_shape(&**s);
    }
    println!("total area = {:.2}", total_area(&shapes));

    // non-void &mut self — exercises @dispatch_mut path
    println!("\n=== try_scale (non-void &mut self) ===");
    for s in &mut shapes {
        let fits = s.try_scale(10.0);
        println!("{:<10} fits in 100×100 box: {fits}", s.name());
    }

    // Works with plain references
    println!("\n=== &dyn Shape ===");
    let circle = Circle { radius: 1.0 };
    let hex = Hexagon { side: 3.0 };
    let refs: Vec<&dyn Shape> = vec![&circle, &hex];
    for s in refs {
        print_shape(s);
    }
}
