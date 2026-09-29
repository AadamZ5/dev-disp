//! The worker background thread that manages the connection and decoder.

use async_tungstenite::tungstenite::Message;
use dev_disp_core::{
    client::run_receiver_with_adapter, host::DisplayParameters, util::PinnedLocalFuture,
};
use dev_disp_encoders::ffmpeg::decoder::{config_file::FfmpegDecoderConfiguration, FfmpegDecoder};
use dev_disp_transports::websocket::receiver::WsReceiver;
use futures::{channel::mpsc, stream::empty, FutureExt, SinkExt, Stream, StreamExt};
use log::{error, warn};
use rust_util::{
    iced_worker_reactor::{iced_worker_reactor, Worker as IcedWorker},
    map_bidirectional::MapBidirectional,
};
use thiserror::Error;

use crate::{util::Runtime, worker::adapter::WsDesktopAdapter};

pub struct Worker<R> {
    runtime: R,
    connect_url: Option<String>,
    event_tx: mpsc::Sender<WorkerEvent>,

    display_parameters: Option<DisplayParameters>,
    display_parameters_tx: Option<mpsc::Sender<DisplayParameters>>,

    _command_tx: mpsc::Sender<WorkerCommand>,
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("Failed to create runtime")]
    RuntimeFailed,
    #[error("Failed to send ready event")]
    ReadyFailed,
    #[error("Failed to connect to the server")]
    ConnectionFailed,
    #[error("Worker loop encountered a failure")]
    LoopFailure,
}

/// An event that the worker produces
#[derive(Debug)]
pub enum WorkerEvent {
    /// The worker subsystem is read, and here is the channel to send commands to it.
    Ready(mpsc::Sender<WorkerCommand>),

    /// Some sort of error encountered by the worker. Not necessarily fatal...
    Error(WorkerError),

    /// Currently connecting to the specified client
    Connecting(String),

    /// Connection succeeded.
    Connected(String),

    /// All connection negotiation has succeeded, and display streaming
    /// will start soon. Here is the facade to the triple buffer for reading
    /// the latest screen data.
    PrepareBuffer(triple_buffer::Output<Vec<u8>>),

    /// The connection has been lost or closed.
    Disconnected,

    /// Hey UI please give us the display parameters.
    RequestDisplayParameters,
}

/// A command that can be given to the worker
#[derive(Debug)]
pub enum WorkerCommand {
    /// The URL to connect the websocket to.
    Connect(String),

    Disconnected,

    /// Set the display parameters that the UI currently has
    SetDisplayParameters(DisplayParameters),
}

impl<R> Worker<R>
where
    R: Runtime,
{
    pub fn connect(&mut self, url: String) -> PinnedLocalFuture<'static, Result<(), WorkerError>> {
        let mut event_tx = self.event_tx.clone();
        self.connect_url = Some(url.clone());

        let (mut display_parameters_tx, display_parameters_rx) =
            mpsc::channel::<DisplayParameters>(16);

        if let Some(display_parameters) = self.display_parameters.clone() {
            match display_parameters_tx.try_send(display_parameters) {
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        "Failed to send initial display parameters to websocket adapter: {:?}",
                        e
                    )
                }
            }
        }

        self.display_parameters_tx = Some(display_parameters_tx);

        async move {
            let (ws, _) = async_tungstenite::tokio::connect_async(url)
                .await
                .map_err(|_| WorkerError::ConnectionFailed)?;

            let ws = MapBidirectional::new(
                ws,
                |item| match item {
                    Ok(Message::Binary(payload)) => Some(payload.to_vec()),
                    _ => None,
                },
                |item| Some(Message::Binary(item.into())),
            );

            let (input, output) = triple_buffer::TripleBuffer::<Vec<u8>>::default().split();

            event_tx
                .send(WorkerEvent::PrepareBuffer(output))
                .await
                .unwrap();

            let ffmpeg_decoder = FfmpegDecoder::new(FfmpegDecoderConfiguration::default());

            let adapter = WsDesktopAdapter::new(display_parameters_rx, input, ffmpeg_decoder);
            let receiver = WsReceiver::new(ws);

            // TODO: Put this on another thread?
            let _ = run_receiver_with_adapter(receiver, adapter).await;

            Ok(())
        }
        .boxed_local()
    }
}

impl<R> Worker<R>
where
    R: crate::util::Runtime,
{
    pub fn new(
        runtime: R,
        event_tx: mpsc::Sender<WorkerEvent>,
    ) -> (Self, mpsc::Receiver<WorkerCommand>) {
        let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>(100);

        let this = Self {
            runtime,
            connect_url: None,
            event_tx,
            _command_tx: command_tx,
            display_parameters_tx: None,
            display_parameters: None,
        };

        (this, command_rx)
    }

    pub async fn send_ready(&mut self) -> Result<(), futures::channel::mpsc::SendError> {
        self.event_tx
            .send(WorkerEvent::Ready(self._command_tx.clone()))
            .await
    }
}

impl<R> IcedWorker<WorkerCommand> for Worker<R>
where
    R: Runtime,
{
    fn initialize(&mut self) -> impl Stream<Item = WorkerCommand> + 'static {
        empty()
    }

    fn update(&mut self, command: WorkerCommand) -> impl Stream<Item = WorkerCommand> + 'static {
        match command {
            WorkerCommand::Connect(url) => {
                let connect_fut = self.connect(url);
                connect_fut
                    .into_stream()
                    .map(|res| match res {
                        Ok(_) => WorkerCommand::Disconnected,
                        Err(_) => WorkerCommand::Disconnected,
                    })
                    .boxed_local()
            }
            WorkerCommand::SetDisplayParameters(display_parameters) => {
                self.display_parameters = Some(display_parameters.clone());
                if let Some(display_parameters_tx) = self.display_parameters_tx.as_mut() {
                    match display_parameters_tx.try_send(display_parameters) {
                        Ok(_) => {}
                        Err(e) => {
                            warn!("Failed to send display parameters: {:?}", e);
                        }
                    }
                }
                empty().boxed_local()
            }
            WorkerCommand::Disconnected => empty().boxed_local(),
        }
    }
}

/// The main entrypoint for the worker.
pub async fn worker_main<R>(
    runtime: R,
    event_sender: mpsc::Sender<WorkerEvent>,
) -> Result<(), WorkerError>
where
    R: crate::util::Runtime,
{
    let (mut worker, command_rx) = Worker::new(runtime, event_sender);
    if let Err(e) = worker.send_ready().await {
        error!("Failed to send ready event: {:?}", e);
        // We should exit since they probably didn't get the communication channel.
        return Err(WorkerError::ReadyFailed);
    }

    iced_worker_reactor(worker, [command_rx])
        .await
        .map_err(|_| WorkerError::LoopFailure)?;

    Ok(())
}
