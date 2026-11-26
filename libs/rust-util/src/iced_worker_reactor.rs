use std::hint::black_box;

use futures::{Stream, StreamExt, stream::SelectAll};

pub trait Worker<T> {
    /// Called once to initialize the worker and return a stream of initial items,
    /// if required.
    fn initialize(&mut self) -> impl Stream<Item = T> + 'static;

    /// Update the worker with a new item and return a stream of processed items.
    fn update(&mut self, item: T) -> impl Stream<Item = T> + 'static;
}

pub async fn iced_worker_reactor<S, Str, T>(
    mut work_state: S,
    seed_streams: impl IntoIterator<Item = Str>,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: Worker<T>,
    Str: Stream<Item = T> + 'static,
{
    let mut tasks = SelectAll::new();
    let initial = work_state.initialize();

    let boxed_seed_streams = std::iter::once(initial.boxed_local())
        .chain(seed_streams.into_iter().map(|s| s.boxed_local()));

    // let boxed_seed_streams = seed_streams.into_iter().map(|s| s.boxed_local());

    tasks.extend(boxed_seed_streams);

    while let Some(item) = tasks.next().await {
        let new_item = work_state.update(item);
        tasks.push(new_item.boxed_local());
    }

    Ok(())
}
