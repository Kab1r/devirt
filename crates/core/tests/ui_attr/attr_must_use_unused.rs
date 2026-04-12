// Verifies that #[must_use] on a trait method is preserved through
// macro expansion: calling compute() without using the result must
// trigger an error under deny(unused_must_use).
#![deny(unused_must_use)]

struct Hot {
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

fn main() {
    let h = Hot { val: 1.0 };
    let d: &dyn Computed = &h;
    d.compute(); // ERROR: unused return value that must be used
}
