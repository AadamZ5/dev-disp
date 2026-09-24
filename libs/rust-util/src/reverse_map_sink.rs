use futures::{Sink, SinkExt};

pub struct ReverseMapSink<S, F, Ti, To> {
    sink: S,
    f: F,
    _marker_out: std::marker::PhantomData<To>,
    _marker_in: std::marker::PhantomData<Ti>,
}

impl<S, F, Ti, To> ReverseMapSink<S, F, Ti, To> {
    pub fn new(sink: S, f: F) -> Self
    where
        S: Sink<To> + Unpin,
        F: Fn(Ti) -> To,
    {
        Self {
            sink,
            f,
            _marker_out: std::marker::PhantomData::<To>,
            _marker_in: std::marker::PhantomData::<Ti>,
        }
    }
}

impl<S, F, Ti, To> Sink<Ti> for ReverseMapSink<S, F, Ti, To>
where
    S: Sink<To> + Unpin,
    F: Fn(Ti) -> To,
{
    type Error = S::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.sink.poll_ready_unpin(cx)
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: Ti) -> Result<(), Self::Error> {
        let this = unsafe { self.get_unchecked_mut() };
        this.sink.start_send_unpin((this.f)(item))
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
