use crate::backend::{ApiFactory, BackendRef, BackendState, Event};
use dev_disp_core::util::PinnedFuture;
use futures::{
    FutureExt, StreamExt,
    channel::mpsc::{self, Sender},
};

pub fn run_backend<T>(
    backend_factory: T,
    event_tx: Sender<Event>,
) -> (BackendRef<T::ConnectParam>, PinnedFuture<'static, ()>)
where
    T: ApiFactory + 'static + Send,
    T::Api: 'static + Send,
{
    let (connect_tx, connect_rx) = mpsc::channel::<T::ConnectParam>(100);
    let (command_tx, command_rx) = mpsc::channel(100);
    let state = BackendState::<T>::new(backend_factory, event_tx);
    let backend_ref = BackendRef::new(command_tx.clone(), connect_tx.clone());

    let backend_task = async move {
        let mut backend_state = state;
        let mut command_stream = command_rx;
        let mut connect_stream = connect_rx;
        loop {
            futures::select! {
                command = command_stream.next().fuse() => {
                    match command {
                        Some(cmd) => {
                            if let Some(event) = backend_state.process_command(cmd).await {
                                backend_state.send_event(event).await;
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
                                backend_state.send_event(Event::Connected(connect_param_clone.to_string())).await;
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
    }.boxed();

    (backend_ref, backend_task)
}
