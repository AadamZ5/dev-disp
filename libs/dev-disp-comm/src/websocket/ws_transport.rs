use async_tungstenite::{WebSocketStream, tungstenite::Message};
use dev_disp_core::{
    client::{ScreenTransport, TransportError},
    core::{DevDispMessageFromClient, DevDispMessageFromSource},
    util::PinnedFuture,
};
use futures::{AsyncRead, AsyncWrite, StreamExt};
use futures_util::FutureExt;

use crate::websocket::messages::{WsMessageFromClient, WsMessageFromSource, WsMessageProtocolInit};

pub struct WsTransport<S> {
    websocket: WebSocketStream<S>,
}

impl<S> WsTransport<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(websocket: WebSocketStream<S>) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        Self { websocket }
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

            let bytes = bincode::serde::encode_to_vec(&req_init, bincode::config::standard())
                .map_err(|e| TransportError::SerializationError)?;
            self.websocket
                .send(Message::binary(bytes))
                .await
                .map_err(|e| TransportError::Other(Box::new(e)))?;

            let m = self
                .websocket
                .next()
                .await
                .ok_or(TransportError::NoConnection)
                .and_then(|msg| match msg {
                    Ok(Message::Binary(bin)) => {
                        let ws_msg: WsMessageFromClient =
                            bincode::serde::decode_from_slice(&bin, bincode::config::standard())
                                .map(|(ws_msg, _)| ws_msg)
                                .map_err(|e| TransportError::SerializationError)?;
                        Ok(ws_msg)
                    }
                    Ok(_) => Err(TransportError::Unknown),
                    Err(e) => Err(TransportError::Other(Box::new(e))),
                });

            match m {
                Ok(WsMessageFromClient::ResponseProtocolInit(resp)) => {
                    if resp.init_key != init_key {
                        return Err(TransportError::Unknown);
                    }
                    Ok(())
                }
                _ => Err(TransportError::Unknown),
            }
        }
        .boxed()
    }

    fn get_display_config(
        &mut self,
    ) -> PinnedFuture<'_, Result<dev_disp_core::host::DisplayParameters, TransportError>> {
        async {
            let req_disp_params =
                WsMessageFromSource::Core(DevDispMessageFromSource::GetDisplayParametersRequest);
            let bytes =
                bincode::serde::encode_to_vec(&req_disp_params, bincode::config::standard())
                    .map_err(|e| TransportError::SerializationError)?;
            self.websocket
                .send(Message::binary(bytes))
                .await
                .map_err(|e| TransportError::Other(Box::new(e)))?;

            let m = self
                .websocket
                .next()
                .await
                .ok_or(TransportError::NoConnection)
                .and_then(|msg| match msg {
                    Ok(Message::Binary(bin)) => {
                        let ws_msg: WsMessageFromClient =
                            bincode::serde::decode_from_slice(&bin, bincode::config::standard())
                                .map(|(ws_msg, _)| ws_msg)
                                .map_err(|e| TransportError::SerializationError)?;
                        Ok(ws_msg)
                    }
                    Ok(_) => Err(TransportError::Unknown),
                    Err(e) => Err(TransportError::Other(Box::new(e))),
                });

            match m {
                Ok(WsMessageFromClient::Core(
                    DevDispMessageFromClient::DisplayParametersUpdate(params),
                )) => Ok(params),
                _ => Err(TransportError::Unknown),
            }
        }
        .boxed()
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> PinnedFuture<'s, Result<(), TransportError>> {
        // TODO: No copy! How can we no-copy in the async below?
        let data = data.to_vec();

        async move {
            let screen_data_msg =
                WsMessageFromSource::Core(DevDispMessageFromSource::PutScreenData(&data));
            let bytes =
                bincode::serde::encode_to_vec(&screen_data_msg, bincode::config::standard())
                    .map_err(|e| TransportError::SerializationError)?;
            self.websocket
                .send(Message::binary(bytes))
                .await
                .map_err(|e| TransportError::Other(Box::new(e)))?;
            Ok(())
        }
        .boxed()
    }
}
