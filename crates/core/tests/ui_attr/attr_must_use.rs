struct Hot {
    val: f64,
}

struct Cold {
    val: f64,
}

#[devirt::devirt(Hot)]
pub trait Computed {
    #[must_use]
    fn compute(&self) -> f64;
}

#[devirt::devirt]
impl Computed for Hot {
    fn compute(&self) -> f64 { self.val * 2.0 }
}

#[devirt::devirt]
impl Computed for Cold {
    fn compute(&self) -> f64 { self.val * 3.0 }
}

fn main() {
    let h = Hot { val: 1.0 };
    let c = Cold { val: 1.0 };
    let dh: &dyn Computed = &h;
    let dc: &dyn Computed = &c;
    let _ = dh.compute(); // should NOT warn (result is used via let _)
    let _ = dc.compute();
}
