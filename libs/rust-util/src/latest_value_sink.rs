use std::{
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
};

use futures::{FutureExt, Stream, StreamExt};
use futures_locks::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct RwLockMappedReadGuard<'a, T, U> {
    _parent_guard: RwLockReadGuard<T>,
    project_fn: Box<dyn Fn(&T) -> &U + 'a>,
}

impl<'a, T, U> RwLockMappedReadGuard<'a, T, U> {
    pub fn new<F>(parent_guard: RwLockReadGuard<T>, project_fn: F) -> Self
    where
        F: Fn(&T) -> &U + 'a,
    {
        Self {
            _parent_guard: parent_guard,
            project_fn: Box::new(project_fn),
        }
    }

    pub fn into_inner(self) -> RwLockReadGuard<T> {
        self._parent_guard
    }
}

impl<'a, T, U> Deref for RwLockMappedReadGuard<'a, T, U> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        (self.project_fn)(&*self._parent_guard)
    }
}

pub(crate) struct LatestValueSinkInner<T> {
    latest_value: Option<T>,
    stream: Pin<Box<dyn Stream<Item = T> + Send>>,
}

impl<T> LatestValueSinkInner<T> {
    fn drain_stream(&mut self) -> bool {
        let mut did_drain = false;
        // Drain the stream synchronously now or never and store the latest value
        while let Some(value) = self.stream.as_mut().next().now_or_never().flatten() {
            self.latest_value = Some(value);
            did_drain = true;
        }
        did_drain
    }
}

#[allow(private_interfaces)]
pub type LatestValueSinkGuard<'a, T> = RwLockMappedReadGuard<'a, LatestValueSinkInner<T>, T>;

pub struct LatestValueSink<T> {
    inner: Arc<RwLock<LatestValueSinkInner<T>>>,
}

impl<T> LatestValueSink<T> {
    pub fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = T> + Send + 'static,
    {
        let inner = LatestValueSinkInner {
            latest_value: None,
            stream: Box::pin(stream),
        };
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    pub async fn drain(&mut self) -> bool {
        let mut inner_guard = self.inner.write().await;
        inner_guard.drain_stream()
    }

    #[allow(private_interfaces)]
    pub async fn get_latest_value_ref(&mut self) -> Option<LatestValueSinkGuard<'_, T>> {
        let mut inner_guard = self.inner.write().await;
        inner_guard.drain_stream();
        let value_present = inner_guard.latest_value.is_some();
        drop(inner_guard);
        let inner_guard = self.inner.read().await;
        if value_present {
            let mapped_guard = RwLockMappedReadGuard::new(inner_guard, |inner| {
                inner.latest_value.as_ref().unwrap()
            });
            Some(mapped_guard)
        } else {
            None
        }
    }

    pub async fn take_latest_value(&mut self) -> Option<T> {
        let mut inner_guard = self.inner.write().await;
        inner_guard.drain_stream();
        inner_guard.latest_value.take()
    }
}

impl<T> LatestValueSink<T>
where
    T: Clone,
{
    pub async fn get_latest_value_cloned(&mut self) -> Option<T> {
        let mut inner_guard = self.inner.write().await;
        inner_guard.drain_stream();
        inner_guard.latest_value.clone()
    }
}

#[cfg(test)]
mod test {

    use super::LatestValueSink;
    use futures::stream;
    use tokio::runtime::Runtime;

    #[test]
    fn test_latest_value_sink() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let values = vec![1, 2, 3, 4, 5];
            let value_stream = stream::iter(values.clone());
            let mut latest_value_sink = LatestValueSink::new(value_stream);

            assert_eq!(latest_value_sink.get_latest_value_cloned().await, Some(5));

            // Take the latest value
            assert_eq!(latest_value_sink.take_latest_value().await, Some(5));

            // After taking, there should be no latest value
            assert_eq!(latest_value_sink.get_latest_value_cloned().await, None);
        });
    }

}