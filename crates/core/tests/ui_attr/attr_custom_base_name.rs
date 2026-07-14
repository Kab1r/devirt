struct Hot {
    value: u32,
}

struct Cold {
    value: u32,
}

#[devirt::devirt(Hot, base = ShapeImpl)]
pub trait Shape {
    fn area(&self) -> u32;
    fn scale(&mut self, factor: u32);
}

#[devirt::devirt]
impl Shape for Hot {
    fn area(&self) -> u32 {
        self.value
    }

    fn scale(&mut self, factor: u32) {
        self.value *= factor;
    }
}

impl ShapeImpl for Cold {
    fn area(&self) -> u32 {
        self.value + 1
    }

    fn scale(&mut self, factor: u32) {
        self.value = self.value * factor + 1;
    }
}

fn main() {
    let mut hot = Hot { value: 2 };
    let cold = Cold { value: 3 };

    assert_eq!((&hot as &dyn Shape).area(), 2);
    assert_eq!((&cold as &dyn Shape).area(), 4);

    (&mut hot as &mut dyn Shape).scale(4);
    assert_eq!(hot.value, 8);
}
