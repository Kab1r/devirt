struct Hot {
    val: f64,
}

#[devirt::devirt(Hot)]
pub trait Checked {
    /// Computes the value.
    #[must_use]
    fn compute(&self) -> f64;
}

#[devirt::devirt]
impl Checked for Hot {
    /// Computes the value.
    #[must_use]
    fn compute(&self) -> f64 { self.val }
}

fn main() {
    let h = Hot { val: 1.0 };
    let r: &dyn Checked = &h;
    assert_eq!(r.compute(), 1.0);
}
