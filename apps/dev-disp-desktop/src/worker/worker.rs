//! The worker background thread that manages the connection and decoder.

use async_tungstenite::tungstenite::Message;
use dev_disp_core::{
    client::run_receiver_with_adapter, host::DisplayParameters, util::PinnedLocalFuture,
};
use dev_disp_encoders::ffmpeg::decoder::{config_file::FfmpegDecoderConfiguration, FfmpegDecoder};
use dev_disp_transports::websocket::receiver::WsReceiver;
use futures::{channel::mpsc, stream::empty, FutureExt, SinkExt, Stream, StreamExt};
use log::{debug, error, info, warn};
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

#[derive(Debug)]
pub struct WorkerRenderContext {
    pub buffer: triple_buffer::Output<Vec<u8>>,
    pub comitted_size: (u32, u32),
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("No display parameters were able to be retreived")]
    NoDisplayParameters,
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
    PrepareRender(WorkerRenderContext),

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

        // If we have display parameters, send them now. If not, request them from the UI
        if let Some(display_parameters) = self.display_parameters.clone() {
            match display_parameters_tx.try_send(display_parameters) {
                Ok(_) => {
                    debug!("Successfully sent initial display parameters to websocket adapter")
                }
                Err(e) => warn!(
                    "Failed to send initial display parameters to websocket adapter: {:?}",
                    e
                ),
            }
        } else {
            debug!("No initial display parameters available for websocket adapter, will request");
            match event_tx.try_send(WorkerEvent::RequestDisplayParameters) {
                Ok(_) => {
                    debug!("Successfully sent RequestDisplayParameters event to websocket adapter")
                }
                Err(e) => warn!(
                    "Failed to send RequestDisplayParameters event to websocket adapter: {:?}",
                    e
                ),
            };
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

            // Allocate 100mb buffer for now
            let size = 1024 * 1024 * 100;

            let (input, output) =
                triple_buffer::TripleBuffer::<Vec<u8>>::new(&vec![0; size]).split();

            let ffmpeg_decoder = FfmpegDecoder::new(FfmpegDecoderConfiguration::default());
            let mut adapter = WsDesktopAdapter::new(display_parameters_rx, input, ffmpeg_decoder);

            // Receive the display parameters that the adapter will use, so we can tell the
            // front end what size the data will be painting as.
            let adapter_display_parameters = match adapter.receive_display_parameters().await {
                Some(dp) => dp,
                None => return Err(WorkerError::NoDisplayParameters),
            };

            // Send render state
            let render_context = WorkerRenderContext {
                buffer: output,
                comitted_size: adapter_display_parameters.resolution,
            };
            event_tx
                .send(WorkerEvent::PrepareRender(render_context))
                .await
                .unwrap();

            // Controller to handle WS transport messages
            let receiver = WsReceiver::new(ws);

            // TODO: Put this on another thread?
            let _ = run_receiver_with_adapter(receiver, adapter).await;

            info!("Receiver finished");

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
                let mut event_tx = self.event_tx.clone();
                connect_fut
                    .into_stream()
                    .map(move |res| match res {
                        Ok(_) => WorkerCommand::Disconnected,
                        Err(e) => {
                            error!("Connect failed: {:?}", e);
                            match event_tx.try_send(WorkerEvent::Error(e)) {
                                Err(e) => error!("Failed to send error event: {:?}", e),
                                _ => {}
                            }
                            return WorkerCommand::Disconnected;
                        }
                    })
                    .boxed_local()
            }
            WorkerCommand::SetDisplayParameters(display_parameters) => {
                debug!(
                    "Worker received display parameters: {:?}",
                    display_parameters
                );
                self.display_parameters = Some(display_parameters.clone());
                if let Some(display_parameters_tx) = self.display_parameters_tx.as_mut() {
                    match display_parameters_tx.try_send(display_parameters) {
                        Ok(_) => {
                            debug!("Successfully sent display parameters to websocket adapter");
                        }
                        Err(e) => {
                            warn!("Failed to send display parameters: {:?}", e);
                        }
                    }
                } else {
                    warn!("Display parameters channel is not available");
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
