use async_tungstenite::{
    WebSocketReceiver, WebSocketSender, WebSocketStream, tungstenite::Message,
};

use dev_disp_core::{
    client::{ScreenTransport, TransportError, TransportSendError, TransportSendMetrics},
    coding::encoder::{
        Encoder, EncoderPossibleCodec, map_external_to_internal_configs,
        map_internal_to_external_configs,
    },
    core::{DevDispMessageFromClient, DevDispMessageFromSource},
    host::{DisplayParameters, ScreenContentParameters},
    util::{PinnedFuture, PinnedLocalFuture},
};
use futures::{AsyncRead, AsyncWrite, SinkExt, StreamExt, channel::mpsc};
use futures_util::FutureExt;
use log::{debug, error, warn};

use crate::websocket::messages::{
    WsMessageDeviceInfo, WsMessageFromClient, WsMessageFromSource, WsMessageProtocolInit,
};

struct BackgroundContext<S> {
    ws_rx: WebSocketReceiver<S>,

    tx_protocol_init: mpsc::Sender<WsMessageProtocolInit>,
    tx_device_info: mpsc::Sender<WsMessageDeviceInfo>,
    tx_core_display_params_update: mpsc::Sender<DisplayParameters>,
    tx_core_preferred_encoding_response: mpsc::Sender<Vec<EncoderPossibleCodec>>,
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
    rx_core_preferred_encoding_response: mpsc::Receiver<Vec<EncoderPossibleCodec>>,
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
        configurations: Vec<EncoderPossibleCodec>,
    ) -> PinnedLocalFuture<'_, Result<Vec<EncoderPossibleCodec>, TransportError>> {
        async move {
            let req_pref_encoding = WsMessageFromSource::RequestPreferredEncodings(configurations);
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
        configuration: EncoderPossibleCodec,
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
                .get_preferred_encodings(external_configs)
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
