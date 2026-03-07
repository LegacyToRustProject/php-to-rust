#[derive(Debug, Clone)]
struct User {
    name: String,
    age: i64,
}

impl User {
    fn new(name: String, age: i64) -> Self {
        Self { name, age }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn age(&self) -> i64 {
        self.age
    }

    fn greet(&self) -> String {
        format!("Hello, I'm {} and I'm {} years old.", self.name, self.age)
    }
}

fn main() {
    let user = User::new("Alice".to_string(), 30);
    print!("{}", user.greet());
}
