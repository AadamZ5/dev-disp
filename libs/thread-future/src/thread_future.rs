use futures::task::AtomicWaker;
use futures_core::FusedFuture;
use pin_project::{pin_project, pinned_drop};
use std::{
    any::Any,
    pin::Pin,
    sync::{Arc, atomic::AtomicBool},
    task::{Context, Poll},
    thread::JoinHandle,
};

pub trait CancellationToken: Clone {
    fn cancel(&self);
}

/// An ultra-simple cancellation token that can be cloned and shared across threads.
#[derive(Debug, Clone)]
pub struct SimpleCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl SimpleCancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(false.into()),
        }
    }

    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl CancellationToken for SimpleCancellationToken {
    fn cancel(&self) {
        Self::cancel(self);
    }
}

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
/// The thread is *not* (and cannot be) terminated when this structure
/// is dropped. The thread must behave nicely and check the
/// cancellation token given to see if it should terminate.
///
/// **Cancellation safety:** If the thread is still running when this future is dropped,
/// the thread will not be forcefully terminated, but the cancellation token will be
/// set so that the thread can check it and terminate early if it wants to. If you
/// want to allow the thread to continue running without setting the cancellation token,
/// you can call [Self::detach_on_drop] or [Self::detach_on_drop_ref] to prevent the
/// cancellation token from being set on drop.
#[pin_project(PinnedDrop)]
pub struct ThreadFuture<T, F, C>
where
    C: CancellationToken + Send + 'static,
{
    /// The inner polling state of the wrapper
    state: ThreadFutureState<T, F>,
    /// `true` if we should cancel the thread when we are
    /// dropped instead of letting it live.
    cancel_on_drop: bool,
    /// A cancellation token shared with the thread
    /// that the thread can check to see if it should stop early.
    cancellation_token: C,
    /// Atomic waker used to communicate to the future when the thread has completed.
    waker: Arc<AtomicWaker>,
}

impl<T, F> ThreadFuture<T, F, SimpleCancellationToken> {
    /// Create a new future-tracked thread using the work function given.
    ///
    /// The thread will be lazily spawned on the first poll of this future.
    ///
    /// The default [SimpleCancellationToken] will be provided to the
    /// thread work function. Check this token to see if the thread
    /// should exit.
    ///
    /// See [Self::new_eager] for eagerly spawning the thread with the default [SimpleCancellationToken].
    /// See [Self::new_with_cancellation] for providing a custom cancellation token.
    pub fn new(work: F) -> Self
    where
        F: (FnOnce(SimpleCancellationToken) -> T) + Send + 'static,
        T: Send + 'static,
    {
        Self {
            state: ThreadFutureState::NotStarted(work),
            cancel_on_drop: true,
            cancellation_token: SimpleCancellationToken::new(),
            waker: Arc::new(AtomicWaker::new()),
        }
    }

    /// Create a new future-tracked thread using the work function given.
    ///
    /// The thread will be eagerly spawned during the call to this function.
    ///
    /// See [Self::new] for lazily spawning the thread with the default [SimpleCancellationToken].
    /// See [Self::new_eager_with_cancellation] for providing a custom cancellation token.
    pub fn new_eager(work: F) -> Self
    where
        F: (FnOnce(SimpleCancellationToken) -> T) + Send + 'static,
        T: Send + 'static,
    {
        let cancellation_token = SimpleCancellationToken::new();
        let waker = Arc::new(AtomicWaker::new());

        let join_handle = Self::spawn_thread(work, cancellation_token.clone(), waker.clone());

        let state = ThreadFutureState::Running(join_handle);

        Self {
            state,
            cancel_on_drop: true,
            cancellation_token,
            waker,
        }
    }
}

impl<T, F, C> ThreadFuture<T, F, C>
where
    F: (FnOnce(C) -> T) + Send + 'static,
    T: Send + 'static,
    C: CancellationToken + Send + 'static,
{
    /// Create a new future-tracked thread using the work function given.
    ///
    /// The thread will be lazily spawned on the first poll of this future.
    ///
    /// Provide a custom cancellation token that implements [CancellationToken]
    /// to share with the thread. The thread can check this token to see if it should
    /// exit.
    ///
    /// See [Self::new_eager] for eagerly spawning the thread with the default [SimpleCancellationToken].
    /// See [Self::new_eager_with_cancellation] for providing a custom cancellation token.
    pub fn new_with_cancellation(work: F, cancellation_token: C) -> Self {
        let waker = Arc::new(AtomicWaker::new());

        Self {
            state: ThreadFutureState::NotStarted(work),
            cancel_on_drop: true,
            cancellation_token,
            waker,
        }
    }

    /// Create a new future-tracked thread using the work function given.
    ///
    /// The thread will be eagerly spawned during the call to this function.
    ///
    /// See [Self::new] for lazily spawning the thread with the default [SimpleCancellationToken].
    /// See [Self::new_with_cancellation] for providing a custom cancellation token.
    pub fn new_eager_with_cancellation(work: F, cancellation_token: C) -> Self {
        let waker = Arc::new(AtomicWaker::new());

        let join_handle = Self::spawn_thread(work, cancellation_token.clone(), waker.clone());

        let state = ThreadFutureState::Running(join_handle);

        Self {
            state,
            cancel_on_drop: true,
            cancellation_token,
            waker,
        }
    }

    /// When called, will instruct the wrapper to not to
    /// activate the cancellation token when dropped.
    pub fn detach_on_drop(mut self) -> Self {
        self.cancel_on_drop = false;
        self
    }

    /// Same as [Self::detach_on_drop], but can be called on a mutable
    /// reference to the future instead of consuming it.
    pub fn detach_on_drop_ref(&mut self) {
        self.cancel_on_drop = false;
    }

    /// Check if the cancellation token will be activated when this future is dropped.
    pub fn is_cancel_on_drop(&self) -> bool {
        self.cancel_on_drop
    }

    /// Get a reference to the cancellation token internally stored.
    pub fn cancellation_token(&self) -> &C {
        &self.cancellation_token
    }

    /// Activate the internal cancellation token.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Internal helper to spawn a thread with the given work function, cancellation token, and waker.
    fn spawn_thread(work: F, cancel_token: C, waker: Arc<AtomicWaker>) -> JoinHandle<T>
    where
        F: (FnOnce(C) -> T) + Send + 'static,
        T: Send + 'static,
    {
        std::thread::spawn(move || {
            let result = work(cancel_token);
            waker.wake();
            result
        })
    }
}

/// If a thread fails to join, this is the error
/// it may return. This can be any value from
/// within the thread panic, so we don't know
/// what it will be.
///
/// See [JoinHandle::join](std::thread::JoinHandle::join) for more details.
type JoinError = Box<dyn Any + Send + 'static>;

impl<T, F, C> Future for ThreadFuture<T, F, C>
where
    F: (FnOnce(C) -> T) + Send + 'static,
    T: Send + 'static,
    C: CancellationToken + Send + 'static,
{
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // Steal the current state, and make sure we replace it
        // in the match statement below.
        let current_state = std::mem::replace(this.state, ThreadFutureState::Polling);

        match current_state {
            ThreadFutureState::NotStarted(work) => {
                // Create a new reference to our internal atomic waker, and
                // register the executor's waker to be notified when the thread
                // completes.
                let waker = this.waker.clone();
                waker.register(cx.waker());
                let cancellation_token = this.cancellation_token.clone();
                let join_handle = Self::spawn_thread(work, cancellation_token, waker);
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
                if !join_handle.is_finished() {
                    // If we haven't finished yet, register the latest waker
                    this.waker.register(cx.waker());
                }

                // After potentially loading that waker, we must check once
                // more if the thread has finished, since it could have finished
                // between the last check and the new waker registration.
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
            ThreadFutureState::Completed => {
                *this.state = ThreadFutureState::Completed;
                Poll::Pending
            }
            ThreadFutureState::Polling => {
                unreachable!(
                    "Intermediate polling state reached, this should not be possible unless the poll function was interrupted during processing!"
                )
            }
        }
    }
}

#[pinned_drop]
impl<T, F, C> PinnedDrop for ThreadFuture<T, F, C>
where
    C: CancellationToken + Send + 'static,
{
    fn drop(self: Pin<&mut Self>) {
        let this = self.project();

        // If we are supposed to cancel the thread on drop, then
        // set the cancellation token so the thread can check it and
        // terminate early if it wants to.
        if *this.cancel_on_drop {
            this.cancellation_token.cancel();
        }
    }
}

/// We know when the future is terminated when the thread
/// has completed, since we will never poll again after that.
///
/// The future is *not* terminated while the thread is still running.
/// This means, even if you activate the cancellation token but the
/// thread has not exited yet, the future is still not terminated.
impl<T, F, C> FusedFuture for ThreadFuture<T, F, C>
where
    F: (FnOnce(C) -> T) + Send + 'static,
    T: Send + 'static,
    C: CancellationToken + Send + 'static,
{
    fn is_terminated(&self) -> bool {
        matches!(self.state, ThreadFutureState::Completed)
    }
}
