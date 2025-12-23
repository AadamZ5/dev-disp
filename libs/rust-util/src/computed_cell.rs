use futures::{FutureExt, Stream, StreamExt, stream::Fuse};
use futures_locks::RwLock;
use shared_stream::{Share, Shared};
use std::{
    fmt::Debug,
    pin::Pin,
    sync::{Arc, atomic::AtomicBool},
};

#[derive(Debug, Clone)]
pub struct ComputedResult<T> {
    pub value: Arc<RwLock<T>>,
    stale: Arc<AtomicBool>,
}

impl<T> ComputedResult<T> {
    pub fn is_stale(&self) -> bool {
        self.stale.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl<T> From<T> for ComputedResult<T> {
    fn from(value: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
            stale: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl<T> ComputedResult<T>
where
    T: Clone,
{
    pub fn get_cloned(&self) -> T {
        // Get the inner cloned value by first cloning the RwLock out of the Arc,
        // such that we've created a new instance of the RwLock to borrow from.
        // Then, we unwrap the RwLock read guard to access the newly cloned inner
        // value.
        match RwLock::clone(&self.value).try_unwrap() {
            Ok(cloned_inner) => cloned_inner,
            Err(_) => panic!("Failed to unwrap newly-cloned RwLock"),
        }
    }
}

struct ComputedCellInner<T, I>
where
    I: Clone,
{
    invalidate_rx: Fuse<Shared<Pin<Box<dyn Stream<Item = I> + Unpin>>>>,
    cached_value: Option<ComputedResult<T>>,
}

impl<T, I> Debug for ComputedCellInner<T, I>
where
    I: Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let opt_text = match &self.cached_value {
            Some(_) => "Some(...)",
            None => "None",
        };

        f.debug_struct("ComputedCellInner")
            .field("cached_value", &opt_text)
            .field("invalidate_rx", &"Stream")
            .finish()
    }
}

impl<T, I> ComputedCellInner<T, I>
where
    I: Clone,
{
    pub fn new<S>(invalidate_rx: S) -> Self
    where
        S: Stream<Item = I> + Unpin + 'static,
    {
        let s = (Box::pin(invalidate_rx) as Pin<Box<dyn Stream<Item = I> + Unpin>>)
            .shared()
            .fuse();

        Self {
            invalidate_rx: s,
            cached_value: None,
        }
    }

    pub fn test_invalidate(&mut self) -> bool {
        if let Some(_) = self.drain_notifications() {
            self.force_invalidate();
            true
        } else {
            false
        }
    }

    pub fn is_valid(&mut self) -> bool {
        self.test_invalidate() && self.cached_value.is_some()
    }

    /// Gets the value, only if one exists
    pub fn get_value(&mut self) -> Option<&ComputedResult<T>> {
        self.cached_value.as_ref()
    }

    /// Forces invalidation by removing the value
    pub fn force_invalidate(&mut self) {
        if let Some(old) = self.cached_value.take() {
            old.stale.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Gets the value if it exists as is valid, or computes a new value using the provided function
    pub async fn get_or_compute_with<F, Fut>(&mut self, compute_fn: F) -> &ComputedResult<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = T>,
    {
        let value = if self.cached_value.is_none() {
            compute_fn().await.into()
        } else {
            self.cached_value.take().unwrap()
        };

        self.cached_value.replace(value);
        self.cached_value.as_ref().unwrap()
    }

    fn drain_notifications(&mut self) -> Option<I> {
        let mut latest: Option<I> = None;
        while let Some(new) = self.invalidate_rx.next().now_or_never().flatten() {
            latest.replace(new);
        }
        latest
    }
}

#[derive(Debug)]
pub struct ComputedCell<T, F, Fut, I>
where
    F: Fn() -> Fut,
    Fut: Future<Output = T>,
    I: Clone,
{
    compute_fn: F,
    inner: ComputedCellInner<T, I>,
}

impl<T, F, Fut, I> ComputedCell<T, F, Fut, I>
where
    F: Fn() -> Fut,
    Fut: Future<Output = T>,
    I: Clone,
{
    pub fn new<S>(compute_fn: F, invalidate_rx: S) -> Self
    where
        S: Stream<Item = I> + Unpin + 'static,
    {
        Self {
            compute_fn,
            inner: ComputedCellInner::new(invalidate_rx),
        }
    }

    pub async fn get(&mut self) -> &ComputedResult<T> {
        self.inner.get_or_compute_with(&self.compute_fn).await
    }

    pub fn get_if_valid(&mut self) -> Option<&ComputedResult<T>> {
        self.inner.test_invalidate();
        self.inner.get_value()
    }

    pub fn invalidate(&mut self) {
        self.inner.force_invalidate();
    }

    pub fn is_valid(&mut self) -> bool {
        self.inner.is_valid()
    }
}

impl<T, F, Fut, I> Clone for ComputedCell<T, F, Fut, I>
where
    F: (Fn() -> Fut) + Clone,
    Fut: Future<Output = T>,
    I: Clone,
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            compute_fn: self.compute_fn.clone(),
            inner: ComputedCellInner {
                invalidate_rx: self.inner.invalidate_rx.get_ref().clone().fuse(),
                cached_value: self.inner.cached_value.clone(),
            },
        }
    }
}
