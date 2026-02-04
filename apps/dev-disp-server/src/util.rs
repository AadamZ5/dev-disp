use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Sink;
use tokio::sync::broadcast;

/// A quick and dirty sink wrapper for tokio's broadcast channel
#[derive(Clone, Debug)]
pub struct BroadcastSink<T>
where
    T: Clone + Send + 'static,
{
    broadcaster: broadcast::Sender<T>,
}

impl<T> BroadcastSink<T>
where
    T: Clone + Send + 'static,
{
    pub fn new(broadcaster: broadcast::Sender<T>) -> Self {
        Self { broadcaster }
    }
}

impl<T> Sink<T> for BroadcastSink<T>
where
    T: Clone + Send + 'static,
{
    type Error = broadcast::error::SendError<T>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        // We don't care if there are no receivers
        self.broadcaster.send(item).map(|_| ())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
