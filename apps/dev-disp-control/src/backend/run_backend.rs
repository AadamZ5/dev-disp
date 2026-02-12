use std::pin::Pin;

use crate::backend::{ApiFactory, BackendRef, BackendState, Event};
use async_stream::stream;
use dev_disp_core::{daemon::api::DeviceCollectionStatus, util::PinnedFuture};
use futures::{
    FutureExt, Stream, StreamExt,
    channel::mpsc::{self, Sender},
    future::{Fuse, FusedFuture},
};

pub fn run_backend<T>(
    backend_factory: T,
) -> (
    BackendRef<T::ConnectParam>,
    impl Stream<Item = Event> + Send,
)
where
    T: ApiFactory + 'static + Send,
    T::Api: 'static + Send,
{
    let (connect_tx, connect_rx) = mpsc::channel::<T::ConnectParam>(100);
    let (command_tx, command_rx) = mpsc::channel(100);
    let state = BackendState::<T>::new(backend_factory);
    let backend_ref = BackendRef::new(command_tx.clone(), connect_tx.clone());

    let event_stream = stream! {

        let mut backend_state = state;
        let mut command_stream = command_rx;
        let mut connect_stream = connect_rx;

        // Disconnect future should resolve when we detect a disconnect. However, to work
        // in the select loop below, we need to start as a never-resolving future.
        // Only on connections will we replace this with a real disconnect future from the backend API.
        let mut disconnect_future: Fuse<PinnedFuture<()>> = futures::future::pending().boxed().fuse();


        // Device update future should resolve when we get a new device update from the backend.
        // Similar to the disconnect future, we start with a never-resolving future and replace it
        // when we are instructed to begin listening to device updates.
        let mut device_update_stream: Option<Pin<Box<dyn Stream<Item = DeviceCollectionStatus> + Send>>> = None;
        let mut device_update_fut: Fuse<PinnedFuture<Option<DeviceCollectionStatus>>> = futures::future::pending().boxed().fuse();

        loop {
            futures::select! {
                _ = disconnect_future => {
                    let event = backend_state.on_disconnect().await;
                    yield event;
                }
                device_update = device_update_fut => {

                    if let Some(device_update) = device_update {
                        log::info!("Received device update from backend: {:?}", device_update);
                        yield Event::DeviceListUpdated(device_update);
                        device_update_fut = device_update_stream.as_mut().map(|stream| stream.next().boxed()).unwrap_or_else(|| futures::future::pending().boxed()).fuse();
                    } else {
                        // When we get a `None`, that means the stream has ended. Remove the
                        // stream.
                        device_update_stream = None;
                        device_update_fut = futures::future::pending().boxed().fuse();
                        log::warn!("Device update stream ended unexpectedly");
                    }
                }
                command = command_stream.next().fuse() => {
                    match command {
                        Some(cmd) => {
                            if let Some(event) = backend_state.process_command(cmd).await {
                                yield event;
                            }
                        }
                        None => {
                            log::info!("Command channel closed, shutting down backend task");
                            break;
                        }
                    }
                }
                connect_param = connect_stream.next().fuse() => {
                    if let Some(connect_param) = connect_param {
                        let connect_param_clone = connect_param.clone();
                        match backend_state.connect(connect_param).await {
                            Ok(_) => {
                                log::info!("Successfully connected to backend");
                                disconnect_future = backend_state.listen_backend_disconnect().boxed().fuse();
                                yield Event::Connected(connect_param_clone.to_string());
                            }
                            Err(e) => {
                                log::error!("Failed to connect to backend: {}", e);
                                // Don't send a disconnected event here, since we were never really connected.
                            }
                        }
                    }
                }
            }
        }

    };

    (backend_ref, event_stream)
}
