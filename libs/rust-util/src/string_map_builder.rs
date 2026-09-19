use std::collections::HashMap;

pub struct StringMapBuilder {
    inner: HashMap<String, String>,
}

impl StringMapBuilder {
    pub fn new() -> Self {
        StringMapBuilder {
            inner: HashMap::new(),
        }
    }

    pub fn insert<T1, T2>(mut self, key: T1, value: T2) -> Self
    where
        T1: Into<String>,
        T2: Into<String>,
    {
        self.inner.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> HashMap<String, String> {
        self.inner
    }
}
