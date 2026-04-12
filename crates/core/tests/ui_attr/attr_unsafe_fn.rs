struct Hot {
    data: *const u8,
}

struct Cold {
    data: *const u8,
}

#[devirt::devirt(Hot)]
pub trait Dangerous {
    unsafe fn deref(&self) -> u8;
}

#[devirt::devirt]
impl Dangerous for Hot {
    unsafe fn deref(&self) -> u8 { unsafe { *self.data } }
}

#[devirt::devirt]
impl Dangerous for Cold {
    unsafe fn deref(&self) -> u8 { unsafe { *self.data } }
}

fn main() {
    let val: u8 = 42;
    let h = Hot { data: &val };
    let c = Cold { data: &val };
    let dh: &dyn Dangerous = &h;
    let dc: &dyn Dangerous = &c;
    assert_eq!(unsafe { dh.deref() }, 42);
    assert_eq!(unsafe { dc.deref() }, 42);
}
