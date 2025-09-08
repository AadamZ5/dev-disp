#[flutter_rust_bridge::frb(sync)] // Synchronous mode for simplicity of the demo
pub fn greet(name: String) -> String {
    format!("Hello, {name}, howreyoudoing???!")
}

pub struct TestStruct {
    pub a: i32,
    pub b: String,
}

pub fn create_test_struct(a: i32, b: String) -> TestStruct {
    TestStruct { a, b }
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}
