struct Hot {
    val: u64,
}

#[devirt::devirt(Hot)]
pub unsafe trait Trusted {
    fn verify(&self) -> bool;
}

// Missing `unsafe` on impl — should fail because __TrustedImpl is unsafe.
#[devirt::devirt]
impl Trusted for Hot {
    fn verify(&self) -> bool { self.val > 0 }
}

fn main() {}
