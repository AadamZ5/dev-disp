use futures::{Sink, SinkExt, Stream, StreamExt};

pub struct Duplex<St, Si, TSi, TSt> {
    stream: St,
    sink: Si,
    _marker: std::marker::PhantomData<(TSi, TSt)>,
}

impl<St, Si, TSi, TSt> Duplex<St, Si, TSi, TSt> {
    pub fn new(stream: St, sink: Si) -> Self
    where
        St: Stream<Item = TSt> + Unpin,
        Si: Sink<TSi> + Unpin,
    {
        Self {
            stream,
            sink,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<St, Si, TSi, TSt> Stream for Duplex<St, Si, TSi, TSt>
where
    St: Stream<Item = TSt> + Unpin,
    Si: Sink<TSi> + Unpin,
{
    type Item = TSt;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.stream.poll_next_unpin(cx)
    }
}

impl<St, Si, TSi, TSt> Sink<TSi> for Duplex<St, Si, TSi, TSt>
where
    St: Stream<Item = TSt> + Unpin,
    Si: Sink<TSi> + Unpin,
{
    type Error = Si::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.sink.poll_ready_unpin(cx)
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: TSi) -> Result<(), Self::Error> {
        let this = unsafe { self.get_unchecked_mut() };
        this.sink.start_send_unpin(item)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.sink.poll_flush_unpin(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.sink.poll_close_unpin(cx)
    }
}
