struct Hot {
    value: u32,
}

struct Cold {
    value: u32,
}

#[devirt::devirt(Hot, devirt = ShapeDevirt)]
pub trait Shape {
    fn area(&self) -> u32;
    fn scale(&mut self, factor: u32);
}

impl Shape for Hot {
    fn area(&self) -> u32 {
        self.value
    }

    fn scale(&mut self, factor: u32) {
        self.value *= factor;
    }
}

impl Shape for Cold {
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
    assert_eq!((&hot as &dyn ShapeDevirt).area(), 2);
    assert_eq!((&cold as &dyn ShapeDevirt).area(), 4);

    (&mut hot as &mut dyn ShapeDevirt).scale(4);
    assert_eq!(hot.value, 8);
}
