struct Bar;

devirt::__devirt_define! {
    @trait []
    pub WrongSig [Bar] {
        fn compute(&self, x: f64) -> f64;
    }
}

devirt::__devirt_define! { @impl [] WrongSig for Bar {
    fn compute(&self, x: u32) -> f64 { f64::from(x) }
}}

fn main() {}
