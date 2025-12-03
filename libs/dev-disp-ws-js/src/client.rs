use std::fmt::Debug;

use dev_disp_comm::websocket::messages::{
    DevDispMessageFromClient, DevDispMessageFromSource, DisplayParameters,
    EncoderPossibleConfiguration, WsMessageFromClient, WsMessageFromSource,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use js_sys::{Promise, SharedArrayBuffer, Uint8Array};
use log::{debug, trace, warn};
use wasm_bindgen::{JsCast, JsError, JsValue};
use wasm_bindgen_futures::JsFuture;
use ws_stream_wasm::WsMessage;

use crate::types::{DevDispEvent, JsDisplayParameters, JsEncoderPossibleConfiguration, WsHandlers};

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
    mut stream: T,
    mut response_tx: S,
    handlers: WsHandlers,
    shared_buffer: Option<SharedArrayBuffer>,
) -> Result<(), JsError>
where
    T: Stream<Item = WsMessage> + Unpin,
    S: Sink<WsMessage> + Unpin,
    S::Error: Debug,
{
    let have_shared_buf = shared_buffer.is_some();
    debug!(
        "WebSocket incoming message listener task starting, shared buffer provided: {}",
        have_shared_buf
    );
    let mut buffer = shared_buffer.unwrap_or_else(|| {
        // Allocate a default SharedArrayBuffer if none was provided
        SharedArrayBuffer::new(512 * 1024 * 1024) // 512 MB
    });

    // I don't know how much memory we could get, but let's allocate enough for 0.5gb

    while let Some(data) = stream.next().await {
        match data {
            WsMessage::Text(text) => {
                warn!(
                    "Received text message over websocket, not supported: {}",
                    text
                );
            }
            WsMessage::Binary(data) => {
                let msg: (WsMessageFromSource, _) =
                    bincode::serde::borrow_decode_from_slice(&data, bincode::config::standard())
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
                        send_ws_message(&mut response_tx, resp).await?;
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
                        send_ws_message(&mut response_tx, device_info).await?;
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
                        send_ws_message(&mut response_tx, resp).await?;
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
                        if let Some(func) = &handlers.on_core {
                            let js_repr =
                                serde_wasm_bindgen::to_value(&dev_disp_message_from_source)
                                    .map_err(|e| {
                                        JsError::new(&format!(
                                            "Failed to convert Core message to JsValue: {:?}",
                                            e
                                        ))
                                    })?;

                            let event = DevDispEvent {
                                error: None,
                                data: Some(js_repr),
                            };
                            let _ = func.call1(&JsValue::NULL, &event.into());
                        }

                        match dev_disp_message_from_source {
                            DevDispMessageFromSource::PutScreenData(screen_data) => {
                                trace!(
                                    "Handling PutScreenData message with {} bytes",
                                    screen_data.len()
                                );

                                let js_val = if have_shared_buf {
                                    // Copy the screen data into the shared buffer
                                    let mut buffer_u8 = Uint8Array::new(&buffer);
                                    buffer_u8
                                        .subarray(0, screen_data.len() as u32)
                                        .copy_from(&screen_data);

                                    JsValue::from(screen_data.len())
                                } else {
                                    // Create a new Uint8Array for the screen data
                                    let uint8_array = Uint8Array::from(&screen_data[..]);
                                    JsValue::from(uint8_array)
                                };

                                let event = DevDispEvent {
                                    error: None,
                                    data: Some(js_val),
                                };
                                let _ = handlers
                                    .handle_screen_data
                                    .call1(&JsValue::NULL, &event.into());
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
                                let params = serde_wasm_bindgen::from_value::<JsDisplayParameters>(
                                    js_value,
                                )?;

                                let real_params: DisplayParameters = params.into();
                                let resp = WsMessageFromClient::Core(
                                    DevDispMessageFromClient::DisplayParametersUpdate(real_params),
                                );
                                send_ws_message(&mut response_tx, resp).await?;
                                debug!("Sent DisplayParametersUpdate message");
                            }
                            DevDispMessageFromSource::GetPreferredEncodingRequest(encodings) => {
                                debug!("Handling GetPreferredEncodingRequest message with {} configurations", encodings.len());
                                let event = encodings
                                    .into_iter()
                                    .filter_map(|config| {
                                        let js_config: JsEncoderPossibleConfiguration = config.into();
                                        match serde_wasm_bindgen::to_value(&js_config) {
                                            Ok(val) => Some(val),
                                            Err(e) => {
                                                warn!(
                                                    "Failed to convert EncoderPossibleConfiguration to JsValue: {:#?}",
                                                    e
                                                );
                                                None
                                            }
                                        }
                                    })
                                    .collect::<js_sys::Array>();

                                let js_value = handlers
                                    .handle_request_preferred_encoding
                                    .call1(&JsValue::NULL, &event.into())
                                    .map_err(|e| {
                                        JsError::new(&format!(
                                            "Failed to call preferred encoding handler: {:?}",
                                            e
                                        ))
                                    })?;

                                let js_fut = js_value
                                    .dyn_into::<Promise>()
                                    .map(|promise| JsFuture::from(promise)).map_err(|e| {
                                    JsError::new(&format!(
                                        "Failed to convert preferred encoding handler result to Promise: {:?}",
                                        e
                                    ))
                                })?;

                                let js_value = js_fut.await.map_err(|e| {
                                    JsError::new(&format!(
                                        "Preferred encoding handler Promise rejected: {:?}",
                                        e
                                    ))
                                })?;

                                debug!("Got preferred encoding from handler: {:?}", js_value);
                                let preferred_encodings =
                                    serde_wasm_bindgen::from_value::<
                                        Vec<JsEncoderPossibleConfiguration>,
                                    >(js_value)?
                                    .into_iter()
                                    .map(|js_config| js_config.into())
                                    .collect::<Vec<EncoderPossibleConfiguration>>();

                                let resp = WsMessageFromClient::Core(
                                    DevDispMessageFromClient::EncodingPreferenceResponse(
                                        preferred_encodings,
                                    ),
                                );
                                send_ws_message(&mut response_tx, resp).await?;
                                debug!("Sent EncodingPreferenceResponse message");
                            }
                            DevDispMessageFromSource::SetEncoding(configuration) => {
                                debug!("Handling SetEncoding message");
                                let js_config: JsEncoderPossibleConfiguration =
                                    configuration.into();
                                let js_value = serde_wasm_bindgen::to_value(&js_config).map_err(|e| {
                                    JsError::new(&format!(
                                        "Failed to convert EncoderPossibleConfiguration to JsValue: {:?}",
                                        e
                                    ))
                                })?;

                                let _ = handlers
                                    .handle_set_encoding
                                    .call1(&JsValue::NULL, &js_value)
                                    .map_err(|e| {
                                        JsError::new(&format!(
                                            "Failed to call set encoding handler: {:?}",
                                            e
                                        ))
                                    })?;
                                debug!("Called set encoding handler");

                                let resp = WsMessageFromClient::Core(
                                    DevDispMessageFromClient::SetEncodingResponse(true),
                                );
                                send_ws_message(&mut response_tx, resp).await?;
                                debug!("Sent SetEncodingResponse message");
                            }
                        }
                    }
                }
            }
        }
    }

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
