use std::pin::Pin;

pub type PinnedFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
