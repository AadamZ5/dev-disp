use crate::backend::{
    ApiFactory, BackendAction, BackendCommand, BackendEvent, BackendRef, BackendState, EventType,
};
use dev_disp_core::util::PinnedStream;
use futures::{
    FutureExt, Stream, StreamExt,
    channel::mpsc::{self},
    select,
    stream::{self, SelectAll},
};

pub fn run_backend<T>(
    backend_factory: T,
) -> (
    BackendRef<T::ConnectParam>,
    impl Stream<Item = BackendEvent> + Send,
)
where
    T: ApiFactory + 'static + Send,
    T::Api: 'static + Send,
{
    let (connect_tx, connect_rx) = mpsc::channel::<T::ConnectParam>(100);
    let (command_tx, command_rx) = mpsc::channel::<BackendCommand>(100);
    let state = BackendState::<T>::new(backend_factory);
    let backend_ref = BackendRef::new(command_tx.clone(), connect_tx.clone());

    let incoming_commands = command_rx.map(|cmd| match cmd {
        BackendCommand::Disconnect => EventType::InternalState(BackendAction::Disconnect),
        BackendCommand::InitializeDevice(a, b) => {
            EventType::InternalState(BackendAction::InitializeDevice(a, b))
        }
        BackendCommand::DisconnectDevice(a, b) => {
            EventType::InternalState(BackendAction::DisconnectDevice(a, b))
        }
        BackendCommand::StreamDevices => EventType::InternalState(BackendAction::StreamDevices),
    });

    let incoming_connect =
        connect_rx.map(|param| EventType::InternalState(BackendAction::Connect(param)));

    struct LoopState<Factory: ApiFactory> {
        backend_state: BackendState<Factory>,
        pending_tasks:
            SelectAll<PinnedStream<'static, EventType<Factory::ConnectParam, Factory::Api>>>,
    }

    let loop_state = LoopState {
        backend_state: state,
        pending_tasks: SelectAll::from_iter([incoming_commands.boxed(), incoming_connect.boxed()]),
    };

    let event_stream = stream::unfold(loop_state, |mut state| async move {
        loop {
            select! {
                event = state.pending_tasks.next().fuse() => {
                    match event {
                        Some(event) => match event {
                            EventType::InternalState(action) => {
                                let new_task = state.backend_state.update(action);
                                state.pending_tasks.push(new_task);
                            }
                            EventType::ToFrontend(event) => {
                                // Just forward frontend events to the frontend.
                                return Some((event, state));
                            }
                        },
                        None => {
                            log::error!("All backend tasks have completed. Backend loop is exiting.");
                            return None;
                        }
                    }
                }
            }
        }

        None
    });

    (backend_ref, event_stream)
}
