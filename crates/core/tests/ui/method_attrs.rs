struct Hot {
    val: f64,
}

devirt::__devirt_define! {
    @trait []
    pub Checked [Hot] {
        /// Computes the value.
        #[must_use]
        fn compute(&self) -> f64;
    }
}

devirt::__devirt_define! { @impl [] Checked for Hot {
    /// Computes the value.
    #[must_use]
    fn compute(&self) -> f64 { self.val }
}}

fn main() {
    let h = Hot { val: 1.0 };
    let r: &dyn Checked = &h;
    assert_eq!(r.compute(), 1.0);
}
