struct Hot {
    val: f64,
}

struct ColdType {
    val: f64,
}

devirt::__devirt_define! {
    @trait []
    pub AllArms [Hot] {
        fn ref_nonvoid(&self, x: f64) -> f64;
        fn ref_void(&self, x: f64);
        fn mut_nonvoid(&mut self, x: f64) -> f64;
        fn mut_void(&mut self, x: f64);
    }
}

devirt::__devirt_define! { @impl [] AllArms for Hot {
    fn ref_nonvoid(&self, x: f64) -> f64 { self.val + x }
    fn ref_void(&self, _x: f64) { }
    fn mut_nonvoid(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn mut_void(&mut self, x: f64) { self.val = x; }
}}

devirt::__devirt_define! { @impl [] AllArms for ColdType {
    fn ref_nonvoid(&self, x: f64) -> f64 { self.val + x }
    fn ref_void(&self, _x: f64) { }
    fn mut_nonvoid(&mut self, x: f64) -> f64 { self.val += x; self.val }
    fn mut_void(&mut self, x: f64) { self.val = x; }
}}

fn main() {
    let mut h: Box<dyn AllArms> = Box::new(Hot { val: 1.0 });
    assert_eq!(h.ref_nonvoid(2.0), 3.0);
    h.ref_void(0.0);
    assert_eq!(h.mut_nonvoid(5.0), 6.0);
    h.mut_void(10.0);

    let mut c: Box<dyn AllArms> = Box::new(ColdType { val: 1.0 });
    assert_eq!(c.ref_nonvoid(2.0), 3.0);
    c.ref_void(0.0);
    assert_eq!(c.mut_nonvoid(5.0), 6.0);
    c.mut_void(10.0);
}
