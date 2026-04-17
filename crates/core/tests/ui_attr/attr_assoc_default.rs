struct Circle;

#[devirt::devirt(Circle)]
pub trait Drawable {
    type Color;
    fn name(&self) -> &str;
    fn draw(&self, color: Self::Color) -> String;
    fn describe(&self) -> String {
        format!("I am {}", self.name())
    }
}

#[devirt::devirt]
impl Drawable for Circle {
    type Color = String;
    fn name(&self) -> &str { "circle" }
    fn draw(&self, color: String) -> String { format!("circle: {color}") }
}

fn main() {
    let c = Circle;
    let d: &dyn Drawable<Color = String> = &c;
    assert_eq!(d.describe(), "I am circle");
}
