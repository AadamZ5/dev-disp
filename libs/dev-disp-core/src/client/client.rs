use std::fmt::Display;

#[derive(Debug)]
pub struct DevDispClient {
    client_id: i32,
    name: String,
}

impl Display for DevDispClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.client_id)
    }
}

impl DevDispClient {
    pub fn new(client_id: i32, name: String) -> Self {
        Self { client_id, name }
    }
}
