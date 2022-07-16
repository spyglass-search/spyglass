use spyglass_plugin::*;

fn main() {
    // basic plugin initialization
    println!("plugin init");
    log();
}

#[no_mangle]
pub fn sum(a: i32, b: i32) -> i32 {
    a + b
}