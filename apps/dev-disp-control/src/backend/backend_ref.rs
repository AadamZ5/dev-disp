use dev_disp_core::util::PinnedFuture;
use futures::{FutureExt, SinkExt, channel::mpsc::Sender};

use crate::backend::BackendCommand;

/// A reference to the backend that can be easily cloned and sent around the application.
/// This is essentially a messaging facade/wrapper to the channel that communicates with
/// the backend task.
#[derive(Debug, Clone)]
pub struct BackendRef<ConnectParam>
where
    ConnectParam: Clone,
{
    command_tx: Sender<BackendCommand>,
    connect_tx: Sender<ConnectParam>,
}

impl<ConnectParam> BackendRef<ConnectParam>
where
    ConnectParam: Clone + std::fmt::Debug + 'static + Send,
{
    pub fn new(command_tx: Sender<BackendCommand>, connect_tx: Sender<ConnectParam>) -> Self {
        Self {
            command_tx,
            connect_tx,
        }
    }

    pub fn send(&self, command: BackendCommand) -> PinnedFuture<'static, ()> {
        let mut sender = self.command_tx.clone();
        async move {
            if let Err(e) = sender.send(command).await {
                log::error!("Failed to send command to backend: {}", e);
            }
        }
        .boxed()
    }

    pub fn connect(&self, endpoint: ConnectParam) -> PinnedFuture<'static, ()> {
        let mut sender = self.connect_tx.clone();
        async move {
            if let Err(e) = sender.send(endpoint).await {
                log::error!("Failed to send connect command to backend: {}", e);
            }
        }
        .boxed()
    }

    pub fn disconnect(&self) -> PinnedFuture<'static, ()> {
        self.send(BackendCommand::Disconnect)
    }
}
