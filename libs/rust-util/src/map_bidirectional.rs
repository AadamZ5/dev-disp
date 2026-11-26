use futures::{Sink, SinkExt, Stream, StreamExt};
use std::marker::PhantomData;

/// A bidirectional map for `Sink + Stream` types (duplex)
///
/// Maps a type T to U in one direction, and U to T in the other.
pub struct MapBidirectional<S, TSt, TSi, U, FSt, FSi> {
    duplex: S,
    stream_map: FSt,
    sink_map: FSi,
    _phantom: std::marker::PhantomData<(TSt, TSi, U)>,
}

impl<S, TSt, TSi, U, FSt, FSi> MapBidirectional<S, TSt, TSi, U, FSt, FSi> {
    pub fn new(duplex: S, stream_map: FSt, sink_map: FSi) -> Self
    where
        S: Sink<TSi> + Stream<Item = TSt> + Unpin,
        FSt: Fn(TSt) -> Option<U>,
        FSi: Fn(U) -> Option<TSi>,
    {
        Self {
            duplex,
            stream_map,
            sink_map,
            _phantom: PhantomData,
        }
    }
}

impl<S, TSt, TSi, U, FSt, FSi> Sink<U> for MapBidirectional<S, TSt, TSi, U, FSt, FSi>
where
    S: Sink<TSi> + Stream<Item = TSt> + Unpin,
    FSt: Fn(TSt) -> Option<U>,
    FSi: Fn(U) -> Option<TSi>,
{
    type Error = S::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.duplex.poll_ready_unpin(cx)
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: U) -> Result<(), Self::Error> {
        let this = unsafe { self.get_unchecked_mut() };
        let mapped_item = (this.sink_map)(item);
        match mapped_item {
            Some(mapped_item) => this.duplex.start_send_unpin(mapped_item),
            None => Ok(()),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.duplex.poll_flush_unpin(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.duplex.poll_close_unpin(cx)
    }
}

impl<S, TSt, TSi, U, FSt, FSi> Stream for MapBidirectional<S, TSt, TSi, U, FSt, FSi>
where
    S: Sink<TSi> + Stream<Item = TSt> + Unpin,
    FSt: Fn(TSt) -> Option<U>,
    FSi: Fn(U) -> Option<TSi>,
{
    type Item = U;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };
        match this.duplex.poll_next_unpin(cx) {
            std::task::Poll::Ready(Some(item)) => {
                match (this.stream_map)(item) {
                    Some(mapped_item) => std::task::Poll::Ready(Some(mapped_item)),
                    None => {
                        // Do not emit this to the consumer, and instead re-wake the waker
                        // in case the stream has another item ready
                        cx.waker().wake_by_ref();
                        std::task::Poll::Pending
                    }
                }
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
