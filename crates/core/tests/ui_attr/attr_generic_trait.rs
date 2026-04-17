struct Handler;

#[devirt::devirt(Handler)]
pub trait Processor<T> {
    fn process(&self, input: T) -> String;
    fn name(&self) -> &str;
}

#[devirt::devirt]
impl Processor<String> for Handler {
    fn process(&self, input: String) -> String { format!("str: {input}") }
    fn name(&self) -> &str { "handler" }
}

#[devirt::devirt]
impl Processor<u32> for Handler {
    fn process(&self, input: u32) -> String { format!("num: {input}") }
    fn name(&self) -> &str { "handler" }
}

fn use_str(p: &dyn Processor<String>) -> String { p.process("hello".into()) }
fn use_u32(p: &dyn Processor<u32>) -> String { p.process(42) }
fn use_send(p: &(dyn Processor<String> + Send)) -> &str { p.name() }

fn main() {
    let h = Handler;
    assert_eq!(use_str(&h), "str: hello");
    assert_eq!(use_u32(&h), "num: 42");
    assert_eq!(use_send(&h), "handler");
}
