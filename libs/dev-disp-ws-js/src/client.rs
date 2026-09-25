use std::fmt::Debug;

use dev_disp_core::client::run_listener_with_adapter;
use dev_disp_transports::websocket::{
    messages::{DevDispMessageFromClient, DisplayParameters, WsMessageFromClient},
    receiver::WsReceiver,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use js_sys::SharedArrayBuffer;
use log::{debug, warn};
use rust_util::{duplex::Duplex, reverse_map_sink::ReverseMapSink};
use wasm_bindgen::JsError;
use ws_stream_wasm::WsMessage;

use crate::{
    adapter::JsAdapter,
    types::{JsDisplayParameters, WsHandlers},
};

/// Helper task that listens to the given dispatcher channels, and
/// sends appropriate message to the WebSocket TX channel/sink.
pub async fn listen_dispatchers<A, S>(
    mut update_display_params_rx: A,
    mut ws_tx: S,
) -> Result<(), JsError>
where
    A: Stream<Item = JsDisplayParameters> + Unpin,
    S: Sink<WsMessage> + Unpin,
    S::Error: Debug,
{
    // TODO: Change to use enums instead of many channels

    while let Some(params) = update_display_params_rx.next().await {
        debug!(
            "Received request to update display parameters to: {:?}",
            params
        );
        let real_params: DisplayParameters = params.into();
        let resp = WsMessageFromClient::Core(DevDispMessageFromClient::DisplayParametersUpdate(
            real_params,
        ));
        send_ws_message(&mut ws_tx, resp).await?;
        debug!("Sent DisplayParametersUpdate message");
    }

    debug!("WebSocket dispatcher listener task ending");

    Ok(())
}

/// Helper task that listens to incoming WebSocket messages on the
/// given channel/stream, and either dispatches a response to the
/// WebSocket TX channel/sink, or calls the appropriate handler.
pub async fn listen_ws_messages<T, S>(
    stream: T,
    response_tx: S,
    handlers: WsHandlers,
    shared_buffer: Option<SharedArrayBuffer>,
) -> Result<(), JsError>
where
    T: Stream<Item = WsMessage> + Unpin,
    S: Sink<WsMessage> + Unpin,
    S::Error: std::error::Error + Send + 'static,
{
    let have_shared_buf = shared_buffer.is_some();
    debug!(
        "WebSocket incoming message listener task starting, shared buffer provided: {}",
        have_shared_buf
    );

    let mapped_rx = stream.filter_map(|msg| match msg {
        WsMessage::Binary(data) => futures::future::ready(Some(data)),
        _ => {
            warn!("Received non-binary WebSocket message, ignoring");
            futures::future::ready(None)
        }
    });

    let mapped_tx = ReverseMapSink::new(response_tx, |msg: Vec<u8>| WsMessage::Binary(msg));

    let duplex = Duplex::new(mapped_rx, mapped_tx);
    let listener = WsReceiver::new(duplex);
    let adapter = JsAdapter::new(handlers, shared_buffer);

    run_listener_with_adapter(listener, adapter).await?;

    debug!("WebSocket incoming message listener task ending");

    Ok(())
}

pub async fn send_ws_message<T>(sink: &mut T, msg: WsMessageFromClient) -> Result<(), JsError>
where
    T: Sink<WsMessage> + Unpin,
    T::Error: Debug,
{
    let bytes = bincode::serde::encode_to_vec(&msg, bincode::config::standard())
        .map_err(|e| JsError::new(&format!("Failed to encode message: {:?}", e)))?;
    sink.send(WsMessage::Binary(bytes))
        .await
        .map_err(|e| JsError::new(&format!("Failed to send message: {:?}", e)))?;
    Ok(())
}
