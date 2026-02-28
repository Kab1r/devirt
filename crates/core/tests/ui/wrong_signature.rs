struct Bar;

devirt::r#trait! {
    pub WrongSig [Bar] {
        fn compute(&self, x: f64) -> f64;
    }
}

devirt::r#impl!(WrongSig for Bar [hot] {
    fn compute(&self, x: u32) -> f64 { f64::from(x) }
});

fn main() {}
