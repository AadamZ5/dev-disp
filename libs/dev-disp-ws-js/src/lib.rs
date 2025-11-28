use std::{net::SocketAddr, ops::DerefMut, panic, str::FromStr};

use bincode::de;
use dev_disp_comm::websocket::messages::{
    DevDispMessageFromClient, DevDispMessageFromSource, DisplayParameters, WsMessageFromClient,
    WsMessageFromSource,
};
use futures::{FutureExt, SinkExt, StreamExt};
use js_sys::Function;
use log::{debug, info, warn};
use wasm_bindgen::prelude::*;
use web_sys::console::{self, debug};
use ws_stream_wasm::{WsMessage, WsMeta, WsStream};

use crate::{
    handlers::{DevDispEvent, JsDisplayParameters, WsHandlers},
    util::OnDrop,
};

mod handlers;
mod util;

// TODO: Please design and think through a better interface here.

#[wasm_bindgen]
pub fn connect_ws(address: &str, handlers: &WsHandlers) -> Result<Function, JsError> {
    // First, parse the given address
    let parsed_address = SocketAddr::from_str(address)
        .map_err(|e| JsError::new(&format!("Invalid address: {}", e)))?;

    // Create cancel channels
    let (cancel_tx, mut cancel_rx) = futures::channel::mpsc::unbounded::<()>();
    let handlers = handlers.clone();

    let _cancel_tx_always_alive = cancel_tx.clone();
    let mut cancel_token = Some(cancel_tx);

    let task = async move {
        info!("Connecting to WebSocket at ws://{}", parsed_address);
        let (ws, mut ws_stream) = WsMeta::connect(&format!("ws://{}", parsed_address), None)
            .await
            .map_err(|e| JsError::new(&format!("Failed to create WebSocket: {:?}", e)))?;

        info!("WebSocket connection established");
        if let Some(func) = &handlers.on_connect {
            let event = DevDispEvent {
                error: None,
                data: None,
            };
            let _ = func.call1(&JsValue::NULL, &event.into());
        }

        while let Some(data) = ws_stream.next().await {
            match data {
                WsMessage::Text(text) => {
                    warn!(
                        "Received text message over websocket, not supported: {}",
                        text
                    );
                }
                WsMessage::Binary(data) => {
                    let msg: (WsMessageFromSource, _) = bincode::serde::borrow_decode_from_slice(
                        &data,
                        bincode::config::standard(),
                    )
                    .map_err(|e| {
                        JsError::new(&format!("Failed to decode binary message: {:?}", e))
                    })?;

                    let msg = msg.0;

                    match msg {
                        WsMessageFromSource::RequestPreInit => {
                            debug!("Received RequestPreInit message");
                            if let Some(func) = &handlers.on_pre_init {
                                let event = DevDispEvent {
                                    error: None,
                                    data: None,
                                };
                                let _ = func.call1(&JsValue::NULL, &event.into());
                            }
                            let resp = WsMessageFromClient::ResponsePreInit;
                            send_ws_message(&mut ws_stream, resp).await?;
                            debug!("Sent ResponsePreInit message");
                            if let Some(func) = &handlers.on_pre_init_success {
                                let event = DevDispEvent {
                                    error: None,
                                    data: None,
                                };
                                let _ = func.call1(&JsValue::NULL, &event.into());
                            }
                        }
                        WsMessageFromSource::RequestDeviceInformation => {
                            debug!("Received RequestDeviceInformation message");
                            let event = DevDispEvent {
                                error: None,
                                data: None,
                            };
                            let _ = handlers
                                .handle_request_device_info
                                .call1(&JsValue::NULL, &event.into());
                            let device_info = WsMessageFromClient::ResponseDeviceInformation(
                                dev_disp_comm::websocket::messages::WsMessageDeviceInfo {
                                    name: "WASM Device".to_string(),
                                    resolution: (800, 600),
                                },
                            );
                            send_ws_message(&mut ws_stream, device_info).await?;
                            debug!("Sent ResponseDeviceInformation message");
                        }
                        WsMessageFromSource::RequestProtocolInit(ws_message_protocol_init) => {
                            debug!(
                                "Received RequestProtocolInit message with key \"{}\"",
                                ws_message_protocol_init.init_key
                            );
                            if let Some(func) = &handlers.on_protocol_init {
                                let event = DevDispEvent {
                                    error: None,
                                    data: None,
                                };
                                let _ = func.call1(&JsValue::NULL, &event.into());
                            }
                            let resp =
                                WsMessageFromClient::ResponseProtocolInit(ws_message_protocol_init);
                            send_ws_message(&mut ws_stream, resp).await?;
                            debug!("Sent ResponseProtocolInit message");
                            if let Some(func) = &handlers.on_protocol_init_success {
                                let event = DevDispEvent {
                                    error: None,
                                    data: None,
                                };
                                let _ = func.call1(&JsValue::NULL, &event.into());
                            }
                        }
                        WsMessageFromSource::Core(dev_disp_message_from_source) => {
                            debug!("Received Core message: {}", dev_disp_message_from_source);

                            let js_repr =
                                serde_wasm_bindgen::to_value(&dev_disp_message_from_source)
                                    .map_err(|e| {
                                        JsError::new(&format!(
                                            "Failed to convert Core message to JsValue: {:?}",
                                            e
                                        ))
                                    })?;

                            if let Some(func) = &handlers.on_core {
                                let event = DevDispEvent {
                                    error: None,
                                    data: Some(js_repr),
                                };
                                let _ = func.call1(&JsValue::NULL, &event.into());
                            }

                            match dev_disp_message_from_source {
                                DevDispMessageFromSource::PutScreenData(screen_data) => {
                                    debug!(
                                        "Handling ScreenData message with {} bytes",
                                        screen_data.len()
                                    );
                                }
                                DevDispMessageFromSource::GetDisplayParametersRequest => {
                                    debug!("Handling GetDisplayParametersRequest message");
                                    let event = DevDispEvent {
                                        error: None,
                                        data: None,
                                    };
                                    let js_value = handlers
                                        .handle_request_display_parameters
                                        .call1(&JsValue::NULL, &event.into())
                                        .map_err(|e| {
                                            JsError::new(&format!(
                                                "Failed to call display parameters handler: {:?}",
                                                e
                                            ))
                                        })?;
                                    debug!("Got display parameters from handler: {:?}", js_value);
                                    let params = serde_wasm_bindgen::from_value::<
                                        JsDisplayParameters,
                                    >(js_value)?;

                                    let real_params: DisplayParameters = params.into();
                                    let resp = WsMessageFromClient::Core(
                                        DevDispMessageFromClient::DisplayParametersUpdate(
                                            real_params,
                                        ),
                                    );
                                    send_ws_message(&mut ws_stream, resp).await?;
                                    debug!("Sent DisplayParametersUpdate message");
                                }
                            }
                        }
                    }
                }
            }
        }

        info!("WebSocket stream was empty.");

        Ok::<(), JsError>(())
    };

    // Spawn the task
    wasm_bindgen_futures::spawn_local(async move {
        let task_fut = task;
        let _cancel_tx_always_alive = _cancel_tx_always_alive;
        let cancel_fut = cancel_rx.next();

        futures::select! {
            res = task_fut.fuse() => {
                if let Err(e) = res {
                    console::error_1(&JsValue::from(e));
                }
            },
            _ = cancel_fut.fuse() => {
                debug!("WebSocket connection cancelled");
            },
        }

        if let Some(func) = &handlers.on_disconnect {
            let event = DevDispEvent {
                error: None,
                data: None,
            };
            let _ = func.call1(&JsValue::NULL, &event.into());
        }
    });

    let mut test = OnDrop::new(
        || {
            debug!("Cancel closure dropped");
        },
        move || {
            // If this has already been called, do nothing.
            if let Some(mut token) = cancel_token.take() {
                debug!("Closing websocket connection");
                let _ = token.send(());
            } else {
                warn!("Websocket connection already closed");
            }
        },
    );

    // Return a closure that when called, will cancel the connection.
    let cancel_closure = Closure::wrap(Box::new(move || {
        test();
    }) as Box<dyn FnMut()>);

    Ok(cancel_closure.into_js_value().into())
}

async fn send_ws_message(
    ws_stream: &mut WsStream,
    msg: WsMessageFromClient,
) -> Result<(), JsError> {
    let bytes = bincode::serde::encode_to_vec(&msg, bincode::config::standard())
        .map_err(|e| JsError::new(&format!("Failed to encode message: {:?}", e)))?;
    ws_stream
        .send(WsMessage::Binary(bytes))
        .await
        .map_err(|e| JsError::new(&format!("Failed to send message: {:?}", e)))?;
    Ok(())
}

#[wasm_bindgen(start)]
pub fn main() -> () {
    console::log_1(&"Initializing logger...".into());
    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));
}
