use async_tungstenite::{
    WebSocketReceiver, WebSocketSender, WebSocketStream, tungstenite::Message,
};

use dev_disp_core::{
    client::{
        ScreenReceiverAdapter, ScreenTransport, ScreenTransportReceiver, TransportError,
        TransportSendError, TransportSendMetrics,
    },
    coding::encoder::{
        CodecOption, Encoder, map_external_to_internal_configs, map_internal_to_external_configs,
    },
    core::{DevDispMessageFromClient, DevDispMessageFromSource},
    host::{DisplayParameters, ScreenContentParameters, ScreenProvider},
    util::{PinnedFuture, PinnedLocalFuture},
};
use futures::{
    AsyncRead, AsyncWrite, Sink, SinkExt, Stream, StreamExt,
    channel::mpsc,
    stream::{SplitSink, SplitStream},
};
use futures_util::FutureExt;
use log::{debug, error, warn};
use thiserror::Error;

use crate::websocket::messages::{
    WsMessageDeviceInfo, WsMessageFromClient, WsMessageFromSource, WsMessageProtocolInit,
};

struct BackgroundContext<S> {
    ws_rx: WebSocketReceiver<S>,

    tx_protocol_init: mpsc::Sender<WsMessageProtocolInit>,
    tx_device_info: mpsc::Sender<WsMessageDeviceInfo>,
    tx_core_display_params_update: mpsc::Sender<DisplayParameters>,
    tx_core_preferred_encoding_response: mpsc::Sender<Vec<CodecOption>>,
    tx_core_set_encoding_response: mpsc::Sender<bool>,
}

pub struct WsTransport<S, E> {
    encoder: E,
    ws_tx: WebSocketSender<S>,
    /// Reciever half of the WebSocket connection. This will be taken
    /// when the background task is started.
    background_context: Option<BackgroundContext<S>>,

    rx_protocol_init: mpsc::Receiver<WsMessageProtocolInit>,
    rx_device_info: mpsc::Receiver<WsMessageDeviceInfo>,

    rx_core_display_params_update: mpsc::Receiver<DisplayParameters>,
    rx_core_preferred_encoding_response: mpsc::Receiver<Vec<CodecOption>>,
    rx_core_set_encoding_response: mpsc::Receiver<bool>,
}

impl<S, E> WsTransport<S, E>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    E: Encoder + 'static,
{
    pub fn new(websocket: WebSocketStream<S>, encoder: E) -> Self {
        let (ws_tx, ws_rx) = websocket.split();

        let (tx_protocol_init, rx_protocol_init) = mpsc::channel(2);
        let (tx_device_info, rx_device_info) = mpsc::channel(2);
        let (tx_core_display_params_update, rx_core_display_params_update) = mpsc::channel(10);
        let (tx_core_preferred_encoding_response, rx_core_preferred_encoding_response) =
            mpsc::channel(2);
        let (tx_core_set_encoding_response, rx_core_set_encoding_response) = mpsc::channel(2);

        let background_ctx = BackgroundContext {
            ws_rx,
            tx_protocol_init,
            tx_device_info,
            tx_core_display_params_update,
            tx_core_preferred_encoding_response,
            tx_core_set_encoding_response,
        };

        Self {
            encoder,
            ws_tx,
            background_context: Some(background_ctx),
            rx_protocol_init,
            rx_device_info,
            rx_core_display_params_update,
            rx_core_preferred_encoding_response,
            rx_core_set_encoding_response,
        }
    }

    async fn send_msg<'a>(&mut self, msg: WsMessageFromSource<'a>) -> Result<(), TransportError> {
        Self::send_msg_with_sender(&mut self.ws_tx, msg).await
    }

    /// Send a message using the provided WebSocket sender, instead of implicitly using the internally
    /// captured one. Useful for separating mutable references (not using `&mut self`)
    async fn send_msg_with_sender<'a, S1>(
        tx: &mut WebSocketSender<S1>,
        msg: WsMessageFromSource<'a>,
    ) -> Result<(), TransportError>
    where
        S1: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        // TODO: Allocate a buffer once and reuse it! Avoid heap allocation on every send
        let bytes = bincode::serde::encode_to_vec(&msg, bincode::config::standard())
            .map_err(|_| TransportError::SerializationError)?;
        tx.send(Message::binary(bytes))
            .await
            .map_err(|e| TransportError::Other(Box::new(e)))?;
        Ok(())
    }

    /// Spawner for the background task
    fn background_task<'a>(&mut self) -> PinnedFuture<'a, Result<(), TransportError>> {
        let background_ctx = self.background_context.take();

        async move {
            let mut background_ctx = background_ctx.ok_or_else(|| TransportError::Unknown)?;

            debug!("Starting WebSocket background task...");

            loop {
                let m = background_ctx
                    .ws_rx
                    .next()
                    .await
                    .ok_or(TransportError::NoConnection)?;

                debug!("Received WebSocket message: {:?}", m);

                match m {
                    Ok(Message::Binary(bin)) => {
                        let _ws_msg =
                            bincode::serde::decode_from_slice(&bin, bincode::config::standard())
                                .map(|(ws_msg, _)| ws_msg);

                        if let Err(e) = _ws_msg {
                            error!("Failed to deserialize WebSocket message: {:?}", e);
                            continue;
                        }

                        match _ws_msg.unwrap() {
                            WsMessageFromClient::ResponseProtocolInit(resp) => {
                                background_ctx
                                    .tx_protocol_init
                                    .send(resp)
                                    .await
                                    .map_err(|e| TransportError::Other(Box::new(e)))?;
                            }
                            WsMessageFromClient::ResponseDeviceInformation(info) => {
                                background_ctx
                                    .tx_device_info
                                    .send(info)
                                    .await
                                    .map_err(|e| TransportError::Other(Box::new(e)))?;
                            }
                            WsMessageFromClient::ResponseSetEncoding(response) => {
                                background_ctx
                                    .tx_core_set_encoding_response
                                    .send(response.success)
                                    .await
                                    .map_err(|e| TransportError::Other(Box::new(e)))?;
                            },
                            WsMessageFromClient::ResponsePreferredEncodings(response) => {
                                background_ctx
                                        .tx_core_preferred_encoding_response
                                        .send(response)
                                        .await
                                        .map_err(|e| TransportError::Other(Box::new(e)))?;
                            }
                            WsMessageFromClient::Core(core_msg) => match core_msg {
                                DevDispMessageFromClient::DisplayParametersUpdate(params) => {
                                    background_ctx
                                        .tx_core_display_params_update
                                        .send(params)
                                        .await
                                        .map_err(|e| TransportError::Other(Box::new(e)))?;
                                }
                            }
                            WsMessageFromClient::ResponsePreInit => {
                                warn!("Received pre-init response when we weren't expecting it... ignoring.");
                            },
                        }
                    }
                    Ok(_) => return Err(TransportError::Unknown),
                    Err(e) => return Err(TransportError::Other(Box::new(e))),
                }
            }
        }
        .boxed()
    }

    fn get_preferred_encodings(
        &mut self,
        screen_parameters: ScreenContentParameters,
        configurations: Vec<CodecOption>,
    ) -> PinnedLocalFuture<'_, Result<Vec<CodecOption>, TransportError>> {
        async move {
            let req_pref_encoding =
                WsMessageFromSource::RequestPreferredEncodings(screen_parameters, configurations);
            debug!("Requesting preferred encoding: {:?}", req_pref_encoding);
            self.send_msg(req_pref_encoding).await?;

            debug!("Waiting for preferred encoding response...");

            self.rx_core_preferred_encoding_response
                .next()
                .await
                .ok_or(TransportError::NoConnection)
        }
        .boxed_local()
    }

    fn set_encoding(
        &mut self,
        configuration: CodecOption,
    ) -> PinnedLocalFuture<'_, Result<(), TransportError>> {
        async move {
            let set_encoding_msg = WsMessageFromSource::SetEncoding(configuration);
            self.send_msg(set_encoding_msg).await?;

            debug!("Waiting for set encoding response...");
            self.rx_core_set_encoding_response
                .next()
                .await
                .ok_or(TransportError::NoConnection)
                .and_then(|success| {
                    if success {
                        Ok(())
                    } else {
                        Err(TransportError::Unknown)
                    }
                })
        }
        .boxed_local()
    }
}

impl<S, E> ScreenTransport for WsTransport<S, E>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    E: Encoder + 'static,
{
    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), TransportError>> {
        async {
            // TODO: Better security!
            let init_key = "yo mamma".to_string();

            // Send initialization message and wait for response
            let req_init = WsMessageFromSource::RequestProtocolInit(WsMessageProtocolInit {
                init_key: init_key.clone(),
            });

            debug!("Sending protocol init message: {:?}", req_init);

            self.send_msg(req_init).await?;

            debug!("Waiting for protocol init response...");

            self.rx_protocol_init
                .next()
                .await
                .ok_or(TransportError::NoConnection)
                .and_then(|resp| {
                    if resp.init_key == init_key {
                        Ok(())
                    } else {
                        Err(TransportError::Unknown)
                    }
                })
        }
        .boxed_local()
    }

    fn background<'a>(&mut self) -> PinnedLocalFuture<'a, Result<(), TransportError>> {
        self.background_task()
    }

    fn get_display_config(
        &mut self,
    ) -> PinnedLocalFuture<'_, Result<dev_disp_core::host::DisplayParameters, TransportError>> {
        async {
            let req_disp_params =
                WsMessageFromSource::Core(DevDispMessageFromSource::GetDisplayParametersRequest);
            debug!("Requesting display parameters: {:?}", req_disp_params);
            self.send_msg(req_disp_params).await?;

            debug!("Waiting for display parameters response...");

            self.rx_core_display_params_update
                .next()
                .await
                .ok_or(TransportError::NoConnection)
        }
        .boxed_local()
    }

    fn prepare_send_screen_data<'s, 'p>(
        &'s mut self,
        source_parameters: &'p ScreenContentParameters,
    ) -> PinnedLocalFuture<'s, Result<(), TransportError>>
    where
        'p: 's,
    {
        async move {
            // Get the internal representation of our encoder options, and then
            // convert them to external representations (no internal data `T`)
            // and ask the client which they can use.
            //
            // Then map that back to our internal repr and tell our encoder to use
            // those configurations.

            let supported_encodings_here = self
                .encoder
                .get_supported_configurations(source_parameters)
                .await
                .map_err(|_| TransportError::Unknown)?;

            let (external_configs, internal_map) =
                map_internal_to_external_configs(supported_encodings_here);

            let client_preferred_encodings = self
                .get_preferred_encodings(source_parameters.clone(), external_configs)
                .await
                .map_err(|_| TransportError::Unknown)?;

            let client_preferred_encodings_internal =
                map_external_to_internal_configs(client_preferred_encodings, &internal_map)
                    .collect::<Vec<_>>();

            let all_encodings_internal = internal_map.values().collect::<Vec<_>>();

            let current_encoding = self
                .encoder
                .set_codec(
                    source_parameters,
                    Some(client_preferred_encodings_internal),
                    all_encodings_internal,
                )
                .await
                .map_err(|_| TransportError::Unknown)?;

            self.set_encoding(current_encoding.for_send(0)).await?;
            Ok(())
        }
        .boxed_local()
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<TransportSendMetrics, TransportSendError>>
    where
        'a: 's,
    {
        async move {
            let encoded_data = self
                .encoder
                .encode(raw_data)
                .await
                .map_err(|e| TransportSendError::EncodeError(Box::new(e)))?;

            let screen_data_msg =
                WsMessageFromSource::Core(DevDispMessageFromSource::PutScreenData(encoded_data));

            Self::send_msg_with_sender(&mut self.ws_tx, screen_data_msg)
                .await
                .map(|_| TransportSendMetrics::Send {
                    sent_bytes: raw_data.len(),
                    send_time: std::time::Duration::from_millis(0),
                })
                .map_err(|e| TransportSendError::SendError(Box::new(e)))
        }
        .boxed_local()
    }
}

pub struct WsReceiver<S> {
    ws_tx: SplitSink<S, WsMessageFromClient>,
    ws_rx: SplitStream<S>,
}

impl<'a, S> WsReceiver<S> {
    pub fn new(duplex: S) -> Self
    where
        S: Sink<WsMessageFromClient> + Stream<Item = WsMessageFromSource<'a>> + Unpin,
    {
        let (ws_tx, ws_rx) = duplex.split();

        Self { ws_tx, ws_rx }
    }
}

pub struct WsReceiverPrepState<'a> {
    tx: mpsc::Sender<WsMessageFromClient>,
    rx: mpsc::Receiver<WsMessageFromSource<'a>>,
    possible_codec_options: Vec<CodecOption>,
}

impl<'a> WsReceiverPrepState<'a> {
    pub fn new(
        codec_options: Vec<CodecOption>,
        tx: mpsc::Sender<WsMessageFromClient>,
        rx: mpsc::Receiver<WsMessageFromSource<'a>>,
    ) -> Self {
        Self {
            tx,
            rx,
            possible_codec_options: codec_options,
        }
    }

    pub fn get_possible_codec_options(&self) -> &Vec<CodecOption> {
        &self.possible_codec_options
    }

    pub async fn send_preferred_codec_options(
        &mut self,
        preferred_codecs: Vec<CodecOption>,
    ) -> Result<(), WsReceiverError> {
        self.tx
            .send(WsMessageFromClient::ResponsePreferredEncodings(
                preferred_codecs,
            ))
            .await
            .map_err(|e| WsReceiverError::SendError(Box::new(e)))
    }

    pub async fn get_final_codec(&mut self) -> Result<CodecOption, WsReceiverError> {
        loop {
            match self.rx.next().await {
                Some(WsMessageFromSource::SetEncoding(codec)) => return Ok(codec),
                Some(_) => {
                    warn!("Unexpected message received while waiting for final codec");
                }
                None => {
                    return Err(WsReceiverError::UnexpectedEnd);
                }
            }
        }
    }
}

/// TODO: We aren't using this yet, but we could require extra functions on the adapter
/// if needed. We are able to fudge this for now.
pub trait WsScreenTransportAdapterExt {
    fn do_pre_init(&mut self) -> PinnedLocalFuture<'_, Result<(), ()>>;
}

#[derive(Debug, Error)]
pub enum WsReceiverError {
    #[error("Transport error: {0}")]
    TransportError(Box<dyn std::error::Error + Send + 'static>),
    #[error("Send error: {0}")]
    SendError(Box<dyn std::error::Error + Send + 'static>),
    #[error("Unexpected end of stream")]
    UnexpectedEnd,
}

impl<'a, S> ScreenTransportReceiver<WsReceiverPrepState<'a>> for WsReceiver<S>
where
    S: Sink<WsMessageFromClient> + Stream<Item = WsMessageFromSource<'a>> + Unpin,
    S::Error: std::error::Error + ScreenProvider + 'static,
{
    type Error = WsReceiverError;

    fn initialize<'s>(&'s mut self) -> PinnedLocalFuture<'s, Result<(), Self::Error>> {
        async { Ok(()) }.boxed_local()
    }

    fn listen<'s, T>(&'s mut self, mut adapter: T) -> PinnedLocalFuture<'s, Result<(), Self::Error>>
    where
        T: ScreenReceiverAdapter<WsReceiverPrepState<'a>> + 's,
        T::Error: std::error::Error + Send + 'static,
    {
        // We use our rx/tx and the adapter to handle the protocol
        async move {
            while let Some(msg) = self.ws_rx.next().await {
                // Handle the incoming message using the adapter
                match msg {
                    WsMessageFromSource::RequestPreInit => {
                        self.ws_tx
                            .send(WsMessageFromClient::ResponsePreInit)
                            .await
                            .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                    }
                    WsMessageFromSource::RequestDeviceInformation => {
                        let display_config = adapter
                            .provide_display_config()
                            .await
                            .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                        self.ws_tx
                            .send(WsMessageFromClient::ResponseDeviceInformation(
                                display_config.into(),
                            ))
                            .await
                            .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                    }
                    WsMessageFromSource::RequestProtocolInit(ws_message_protocol_init) => {
                        // TODO: Better initialization phase! Security!
                        self.ws_tx
                            .send(WsMessageFromClient::ResponseProtocolInit(
                                ws_message_protocol_init,
                            ))
                            .await
                            .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                    }
                    WsMessageFromSource::Core(dev_disp_message_from_source) => {
                        match dev_disp_message_from_source {
                            DevDispMessageFromSource::GetDisplayParametersRequest => {
                                let display_config = adapter
                                    .provide_display_config()
                                    .await
                                    .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                                self.ws_tx
                                    .send(WsMessageFromClient::Core(
                                        DevDispMessageFromClient::DisplayParametersUpdate(
                                            display_config,
                                        ),
                                    ))
                                    .await
                                    .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                            }
                            DevDispMessageFromSource::PutScreenData(items) => {
                                adapter
                                    .recieve_screen_data(&items)
                                    .await
                                    .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                            }
                        }
                    }
                    WsMessageFromSource::RequestPreferredEncodings(
                        screen_content_params,
                        codec_options,
                    ) => {
                        // This is fucking dumb. Because of E0276, we cannot properly attach our
                        // state as a GAT of the `ScreenTransportReceiver` trait, which means
                        // we can't properly specify lifetimes to borrow the websocket tx/rx handles,
                        // so we have to do a stupid stupid captive message handling here to facilitate
                        // the handshake instead of the state just taking the handles directly.
                        //
                        // So fucking stupid.
                        // https://github.com/rust-lang/rust/issues/134890

                        let (ws_send_tx, mut ws_send_rx) = mpsc::channel(8);
                        let (mut ws_recieve_tx, ws_receive_rx) = mpsc::channel(8);

                        let ws_tx = &mut self.ws_tx;

                        let fw_outbound = async move {
                            while let Ok(mxg) = ws_send_rx.recv().await {
                                ws_tx
                                    .send(mxg)
                                    .await
                                    .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                            }

                            Ok::<(), WsReceiverError>(())
                        };

                        let ws_rx = &mut self.ws_rx;

                        let fw_inbound = async move {
                            while let Some(mxg) = ws_rx.next().await {
                                ws_recieve_tx
                                    .send(mxg)
                                    .await
                                    .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                            }

                            Ok::<(), WsReceiverError>(())
                        };

                        let prep_state =
                            WsReceiverPrepState::new(codec_options, ws_send_tx, ws_receive_rx);
                        let negotiation =
                            adapter.prepare_receive_screen_data(&screen_content_params, prep_state);

                        // Poll everything until the negotiation task completes.
                        // The other two shouldn't complete until the negotiation task completes and drops
                        // the prep state.
                        futures::select_biased! {
                            rslt = negotiation.fuse() => {
                                rslt.map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                            }
                            fw_out_rslt = fw_outbound.fuse() => {
                                fw_out_rslt?;
                            }
                            fw_in_rslt = fw_inbound.fuse() => {
                                fw_in_rslt?;
                            }
                        }

                        // JFC that sucks.
                    }
                    WsMessageFromSource::SetEncoding(_) => {
                        // We should not get here, since codec negotiation happens
                        // in the WsReceiverPrepState negotiation phase.
                        error!("Received SetEncoding message outside of negotiation phase");
                    }
                }
            }

            Ok(())
        }
        .boxed_local()
    }
}
