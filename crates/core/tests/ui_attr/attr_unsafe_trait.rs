struct Hot {
    val: u64,
}

struct Cold {
    val: u64,
}

#[devirt::devirt(Hot)]
pub unsafe trait Trusted {
    fn verify(&self) -> bool;
}

#[devirt::devirt]
unsafe impl Trusted for Hot {
    fn verify(&self) -> bool { self.val > 0 }
}

#[devirt::devirt]
unsafe impl Trusted for Cold {
    fn verify(&self) -> bool { self.val != 0 }
}

fn check(t: &dyn Trusted) -> bool {
    t.verify()
}

fn main() {
    let h = Hot { val: 42 };
    let c = Cold { val: 1 };
    assert!(check(&h));
    assert!(check(&c));
}
