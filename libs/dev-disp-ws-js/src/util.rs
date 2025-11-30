use std::ops::{Deref, DerefMut};

pub struct OnDropFn<F>
where
    F: FnOnce(),
{
    on_drop: Option<F>,
}

impl<F> OnDropFn<F>
where
    F: FnOnce(),
{
    pub fn new(func: F) -> Self {
        Self {
            on_drop: Some(func),
        }
    }
}

impl<F> Drop for OnDropFn<F>
where
    F: FnOnce(),
{
    fn drop(&mut self) {
        if let Some(func) = self.on_drop.take() {
            func();
        }
    }
}

pub struct OnDrop<F, T>
where
    F: FnOnce(),
{
    on_drop: Option<F>,
    value: T,
}

impl<F, T> OnDrop<F, T>
where
    F: FnOnce(),
{
    pub fn new(func: F, value: T) -> Self {
        Self {
            on_drop: Some(func),
            value,
        }
    }
}

impl<F, T> Drop for OnDrop<F, T>
where
    F: FnOnce(),
{
    fn drop(&mut self) {
        if let Some(func) = self.on_drop.take() {
            func();
        }
    }
}

impl<F, T> Deref for OnDrop<F, T>
where
    F: FnOnce(),
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<F, T> DerefMut for OnDrop<F, T>
where
    F: FnOnce(),
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
