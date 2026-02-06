use crate::backend::Command;
use futures::{SinkExt, channel::mpsc::Sender};

#[derive(Debug, Clone)]
pub struct BackendRef<ConnectParam>
where
    ConnectParam: Clone,
{
    command_tx: Sender<Command>,
    connect_tx: Sender<ConnectParam>,
}

impl<ConnectParam> BackendRef<ConnectParam>
where
    ConnectParam: Clone + std::fmt::Debug + 'static + Send,
{
    pub fn new(command_tx: Sender<Command>, connect_tx: Sender<ConnectParam>) -> Self {
        Self {
            command_tx,
            connect_tx,
        }
    }

    pub fn send(&mut self, command: Command) {
        let mut sender = self.command_tx.clone();
        // TODO: Don't block?
        iced::futures::executor::block_on(async move {
            if let Err(e) = sender.send(command).await {
                log::error!("Failed to send command to backend: {}", e);
            }
        });
    }

    pub fn connect(&mut self, endpoint: ConnectParam) {
        let mut sender = self.connect_tx.clone();
        // TODO: Don't block?
        iced::futures::executor::block_on(async move {
            if let Err(e) = sender.send(endpoint).await {
                log::error!("Failed to send connect command to backend: {}", e);
            }
        });
    }

    pub fn disconnect(&mut self) {
        self.send(Command::Disconnect);
    }
}
