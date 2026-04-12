struct Inner {
    val: i32,
}

devirt::__devirt_define! {
    @trait
    /// A public trait with documentation.
    pub DocTrait [Inner] {
        /// Returns the inner value.
        fn get(&self) -> i32;
    }
}

devirt::__devirt_define! { @impl DocTrait for Inner {
    fn get(&self) -> i32 { self.val }
}}

fn main() {
    let d: Box<dyn DocTrait> = Box::new(Inner { val: 42 });
    assert_eq!(d.get(), 42);
}
