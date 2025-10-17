// Test file for syntax highlighting
use std::collections::HashMap;

fn main() {
    let mut map = HashMap::new();
    map.insert("key", "value");

    println!("Hello, world!");

    for i in 0..10 {
        if i % 2 == 0 {
            println!("Even: {}", i);
        } else {
            println!("Odd: {}", i);
        }
    }
}

struct Person {
    name: String,
    age: u32,
}

impl Person {
    fn new(name: String, age: u32) -> Self {
        Person { name, age }
    }
}
