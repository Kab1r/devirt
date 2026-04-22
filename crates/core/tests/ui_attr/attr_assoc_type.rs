struct Circle;
struct Rect;

#[devirt::devirt(Circle)]
pub trait Drawable {
    type Color;
    fn name(&self) -> &str;
    fn draw(&self, color: Self::Color) -> String;
}

#[devirt::devirt]
impl Drawable for Circle {
    type Color = String;
    fn name(&self) -> &str { "circle" }
    fn draw(&self, color: String) -> String { format!("circle: {color}") }
}

#[devirt::devirt]
impl Drawable for Rect {
    type Color = u32;
    fn name(&self) -> &str { "rect" }
    fn draw(&self, color: u32) -> String { format!("rect: #{color:06x}") }
}

fn check_name(d: &dyn Drawable<Color = String>) -> &str { d.name() }
fn check_draw(d: &dyn Drawable<Color = String>, c: String) -> String { d.draw(c) }
fn check_send(d: &(dyn Drawable<Color = String> + Send)) -> &str { d.name() }

fn main() {
    let c = Circle;
    assert_eq!(check_name(&c), "circle");
    assert_eq!(check_draw(&c, "red".into()), "circle: red");
    assert_eq!(check_send(&c), "circle");

    let r = Rect;
    let d: &dyn Drawable<Color = u32> = &r;
    assert_eq!(d.name(), "rect");
    assert_eq!(d.draw(0x00FF_0000), "rect: #ff0000");
}
