struct Hot;

#[devirt::devirt(Hot)]
trait Conv {
    type Out;
    fn nop(&self);
}

impl ConvBase for Hot {
    type Out = ();
    fn nop(&self) {}
}

impl ConvBase for dyn Conv<Out = usize> {
    type Out = &'static String;
    fn nop(&self) {}
}

fn launder<T: ?Sized + ConvBase>(x: <T as __ConvImpl>::Out) -> <T as ConvBase>::Out {
    x
}

fn main() {
    let s: &'static String = launder::<dyn Conv<Out = usize>>(0x4141_4141);
    let _ = s.len();
}
