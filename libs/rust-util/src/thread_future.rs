use pin_project::{pin_project, pinned_drop};
use std::{
    any::Any,
    pin::Pin,
    task::{Context, Poll},
    thread::JoinHandle,
};

#[pin_project(project = ThreadFutureStateProj)]
pub enum ThreadFutureState<T, F> {
    /// The thread has not been started yet, the
    /// work function is waiting
    NotStarted(#[pin] F),

    /// The thread is running.
    Running(JoinHandle<T>),

    /// The thread completed or failed, the value was
    /// returned already. We have nothing to do.
    Completed,

    /// Internal state where the poll state machine
    /// is being computed.
    Polling,
}

/// Create a future that wraps a thread. The thread is lazily created.
/// The thread is *not* terminated when this structure is dropped.
#[pin_project]
pub struct ThreadFuture<T, F> {
    /// The inner polling state of the wrapper
    state: ThreadFutureState<T, F>,
    /// `true` if we should kill the thread when we are
    /// dropped instead of letting it live.
    kill_on_drop: bool,
}

impl<T, F> ThreadFuture<T, F> {
    pub fn new(work: F) -> Self
    where
        F: (FnOnce() -> T) + Send + 'static,
        T: Send + 'static,
    {
        Self {
            state: ThreadFutureState::NotStarted(work),
            kill_on_drop: true,
        }
    }

    /// When called, will instruct the wrapper to not kill the
    /// inner thread when we are dropped.
    pub fn detach_on_drop(mut self) -> Self {
        self.kill_on_drop = false;
        self
    }
}

/// If a thread fails to join, this is the error
/// it may return.
type JoinError = Box<dyn Any + Send + 'static>;

impl<T, F> Future for ThreadFuture<T, F>
where
    F: (FnOnce() -> T) + Send + 'static,
    T: Send + 'static,
{
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // Steal the current state, and make sure we replace it
        // in the match statement below.
        let current_state = std::mem::replace(this.state, ThreadFutureState::Polling);

        match current_state {
            ThreadFutureState::NotStarted(work) => {
                let waker = cx.waker().clone();
                let join_handle = std::thread::spawn(|| {
                    let result = work();
                    waker.wake();
                    result
                });
                *this.state = ThreadFutureState::Running(join_handle);
                Poll::Pending
            }
            ThreadFutureState::Running(join_handle) => {
                // In our implementation, we shouldn't be polled again until
                // the thread wakes via the waker we copied to it. However,
                // we can't assume all async runtimes will be nice and wait
                // to poll us again, so if we are polled again before
                // we've used the waker, make sure we don't try to join
                // too soon.
                if join_handle.is_finished() {
                    *this.state = ThreadFutureState::Completed;
                    return Poll::Ready(join_handle.join());
                } else {
                    // Move the state back in for the next poll
                    *this.state = ThreadFutureState::Running(join_handle);
                    return Poll::Pending;
                }
            }
            // If we get polled after we completed, we will forever be
            // pending.
            ThreadFutureState::Completed => Poll::Pending,
            ThreadFutureState::Polling => {
                unreachable!(
                    "Intermediate polling state reached, this should not be possible unless the poll function was interrupted"
                )
            }
        }
    }
}
