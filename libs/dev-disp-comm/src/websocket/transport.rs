use std::io::Read;

use async_tungstenite::{
    WebSocketReceiver, WebSocketSender, WebSocketStream, tungstenite::Message,
};

use dev_disp_core::{
    client::{ScreenTransport, TransportError},
    core::{DevDispMessageFromClient, DevDispMessageFromSource},
    host::DisplayParameters,
    util::PinnedFuture,
};
use futures::{AsyncRead, AsyncWrite, SinkExt, StreamExt, channel::mpsc};
use futures_util::FutureExt;
use log::{debug, error};

use crate::websocket::messages::{
    WsMessageDeviceInfo, WsMessageFromClient, WsMessageFromSource, WsMessageProtocolInit,
};

struct BackgroundContext<S> {
    ws_rx: WebSocketReceiver<S>,

    tx_protocol_init: mpsc::Sender<WsMessageProtocolInit>,
    tx_device_info: mpsc::Sender<WsMessageDeviceInfo>,
    tx_core_display_params_update: mpsc::Sender<DisplayParameters>,
}

pub struct WsTransport<S> {
    ws_tx: WebSocketSender<S>,
    /// Reciever half of the WebSocket connection. This will be taken
    /// when the background task is started.
    background_context: Option<BackgroundContext<S>>,

    rx_protocol_init: mpsc::Receiver<WsMessageProtocolInit>,
    rx_device_info: mpsc::Receiver<WsMessageDeviceInfo>,

    rx_core_display_params_update: mpsc::Receiver<DisplayParameters>,
}

impl<S> WsTransport<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(websocket: WebSocketStream<S>) -> Self {
        let (ws_tx, ws_rx) = websocket.split();

        let (tx_protocol_init, rx_protocol_init) = mpsc::channel(100);
        let (tx_device_info, rx_device_info) = mpsc::channel(100);
        let (tx_core_display_params_update, rx_core_display_params_update) = mpsc::channel(100);

        let background_ctx = BackgroundContext {
            ws_rx,
            tx_protocol_init,
            tx_device_info,
            tx_core_display_params_update,
        };

        Self {
            ws_tx,
            background_context: Some(background_ctx),
            rx_protocol_init,
            rx_device_info,
            rx_core_display_params_update,
        }
    }

    async fn send_msg<'a>(&mut self, msg: WsMessageFromSource<'a>) -> Result<(), TransportError> {
        // TODO: Allocate a buffer once and reuse it! Avoid heap allocation on every send
        let bytes = bincode::serde::encode_to_vec(&msg, bincode::config::standard())
            .map_err(|e| TransportError::SerializationError)?;
        self.ws_tx
            .send(Message::binary(bytes))
            .await
            .map_err(|e| TransportError::Other(Box::new(e)))?;
        Ok(())
    }

    fn _background_task<'s, 'a>(&'s mut self) -> PinnedFuture<'a, Result<(), TransportError>> {
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
                                let _ = background_ctx
                                    .tx_protocol_init
                                    .send(resp)
                                    .await
                                    .map_err(|e| TransportError::Other(Box::new(e)))?;
                            }
                            WsMessageFromClient::ResponseDeviceInformation(info) => {
                                let _ = background_ctx
                                    .tx_device_info
                                    .send(info)
                                    .await
                                    .map_err(|e| TransportError::Other(Box::new(e)))?;
                            }
                            WsMessageFromClient::Core(core_msg) => match core_msg {
                                DevDispMessageFromClient::DisplayParametersUpdate(params) => {
                                    let _ = background_ctx
                                        .tx_core_display_params_update
                                        .send(params)
                                        .await
                                        .map_err(|e| TransportError::Other(Box::new(e)))?;
                                }
                                _ => {
                                    debug!(
                                        "Received unhandled core message from client: {:?}",
                                        core_msg
                                    );
                                }
                            },
                            other => {
                                error!("Received unexpected WebSocket message {:?}", other);
                                continue;
                            }
                        }
                    }
                    Ok(_) => return Err(TransportError::Unknown),
                    Err(e) => return Err(TransportError::Other(Box::new(e))),
                }
            }
        }
        .boxed()
    }
}

impl<S> ScreenTransport for WsTransport<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn initialize(&mut self) -> PinnedFuture<'_, Result<(), TransportError>> {
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
        .boxed()
    }

    fn background<'s, 'a>(&'s mut self) -> PinnedFuture<'a, Result<(), TransportError>> {
        self._background_task()
    }

    fn get_display_config(
        &mut self,
    ) -> PinnedFuture<'_, Result<dev_disp_core::host::DisplayParameters, TransportError>> {
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
        .boxed()
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> PinnedFuture<'s, Result<(), TransportError>>
    where
        'a: 's,
    {
        async move {
            let screen_data_msg = WsMessageFromSource::Core(
                DevDispMessageFromSource::PutScreenData(data[0..64 * 1024].as_ref()),
            );
            self.send_msg(screen_data_msg).await
        }
        .boxed()
    }
}
