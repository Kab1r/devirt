struct Widget {
    x: f64,
    y: f64,
}

devirt::__devirt_define! {
    @trait
    pub MultiArg [Widget] {
        fn add(&self, a: f64, b: f64) -> f64;
        fn set(&mut self, a: f64, b: f64);
    }
}

devirt::__devirt_define! { @impl MultiArg for Widget {
    fn add(&self, a: f64, b: f64) -> f64 { self.x + a + self.y + b }
    fn set(&mut self, a: f64, b: f64) { self.x = a; self.y = b; }
}}

fn main() {
    let mut w: Box<dyn MultiArg> = Box::new(Widget { x: 1.0, y: 2.0 });
    assert_eq!(w.add(3.0, 4.0), 10.0);
    w.set(5.0, 6.0);
    assert_eq!(w.add(0.0, 0.0), 11.0);
}
