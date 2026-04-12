struct Hot { val: u64 }
struct Cold { val: u64 }

#[devirt::devirt(Hot)]
pub trait Shape {
    fn area(&self) -> f64;
    fn scale(&mut self, factor: f64);
}

#[devirt::devirt]
impl Shape for Hot {
    fn area(&self) -> f64 { self.val as f64 }
    fn scale(&mut self, factor: f64) { self.val = (self.val as f64 * factor) as u64; }
}

#[devirt::devirt]
impl Shape for Cold {
    fn area(&self) -> f64 { self.val as f64 + 1.0 }
    fn scale(&mut self, factor: f64) { self.val = (self.val as f64 * factor) as u64 + 1; }
}

fn use_ref(s: &dyn Shape) -> f64 { s.area() }
fn use_send_ref(s: &(dyn Shape + Send)) -> f64 { s.area() }
fn use_sync_ref(s: &(dyn Shape + Sync)) -> f64 { s.area() }
fn use_send_sync_ref(s: &(dyn Shape + Send + Sync)) -> f64 { s.area() }

fn use_send_mut(s: &mut (dyn Shape + Send), f: f64) { s.scale(f); }

fn use_boxed_send(s: Box<dyn Shape + Send>) -> f64 { s.area() }

fn main() {
    let h = Hot { val: 42 };

    assert_eq!(use_ref(&h), 42.0);
    assert_eq!(use_send_ref(&h), 42.0);
    assert_eq!(use_sync_ref(&h), 42.0);
    assert_eq!(use_send_sync_ref(&h), 42.0);

    // Cold type — falls through to vtable
    let c = Cold { val: 42 };
    assert_eq!(use_send_ref(&c), 43.0);
    assert_eq!(use_sync_ref(&c), 43.0);
    assert_eq!(use_send_sync_ref(&c), 43.0);

    // &mut self through dyn Trait + Send
    let mut h2 = Hot { val: 10 };
    use_send_mut(&mut h2, 2.0);
    assert_eq!(h2.val, 20);

    // Box<dyn Trait + Send>
    let boxed: Box<dyn Shape + Send> = Box::new(Hot { val: 5 });
    assert_eq!(use_boxed_send(boxed), 5.0);
}
