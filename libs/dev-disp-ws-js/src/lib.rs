use std::{net::SocketAddr, panic, str::FromStr};

use futures::{channel::mpsc, stream::FuturesUnordered, FutureExt, SinkExt, StreamExt};
use log::{debug, error, info, warn};
use wasm_bindgen::prelude::*;
use web_sys::OffscreenCanvas;
use ws_stream_wasm::{WsMessage, WsMeta};

use crate::{
    client::{listen_dispatchers, listen_ws_messages},
    types::{DevDispEvent, JsDisplayParameters, WsDispatchers, WsHandlers},
    util::OnDrop,
};

mod client;
mod types;
mod util;

// TODO: Please design and think through a better interface here.

/// Connect to a DevDisp server at the given address, and set up
/// the appropriate handlers and canvas for rendering.
/// Returns a set of dispatchers for controlling the connection.
#[wasm_bindgen(js_name = "connectDevDispServer")]
pub fn connect_dev_disp_server(
    address: &str,
    handlers: &WsHandlers,
    canvas: OffscreenCanvas,
) -> Result<WsDispatchers, JsError> {
    // First, parse the given address
    let parsed_address = SocketAddr::from_str(address)
        .map_err(|e| JsError::new(&format!("Invalid address: {}", e)))?;

    // Create cancel channels
    let (cancel_tx, mut cancel_rx) = mpsc::unbounded::<()>();
    let handlers = handlers.clone();
    let handlers_1 = handlers.clone();

    let (update_display_params_tx, update_display_params_rx) =
        mpsc::unbounded::<JsDisplayParameters>();

    let mut closed = false;

    let _cancel_tx_always_alive = cancel_tx.clone();
    let mut cancel_token = cancel_tx.clone();
    let mut cancel_token_outer = cancel_tx.clone();

    let task_main = async move {
        let handlers = handlers_1;
        info!("Connecting to WebSocket at ws://{}", parsed_address);
        let (_, ws_stream) = WsMeta::connect(&format!("ws://{}", parsed_address), None)
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

        let (ws_fwd_tx, mut ws_fwd_rx) = mpsc::channel::<WsMessage>(100);
        let (ws_tx_original, ws_rx) = ws_stream.split();

        let task_rx_update_display_params =
            listen_dispatchers(update_display_params_rx, ws_fwd_tx.clone()).boxed_local();
        let task_rx = listen_ws_messages(ws_rx, ws_fwd_tx, handlers.clone())
            .then(|r| async move {
                // Call cancel token
                let _ = cancel_token.send(()).await;

                r
            })
            .boxed_local();

        let task_forward_tx = async move {
            let mut ws_tx = ws_tx_original;
            while let Some(msg) = ws_fwd_rx.next().await {
                ws_tx
                    .send(msg)
                    .await
                    .map_err(|e| JsError::new(&format!("Failed to forward WS message: {:?}", e)))?;
            }

            debug!("WebSocket outgoing message task ending");

            Ok::<(), JsError>(())
        }
        .boxed_local();

        let mut futures = FuturesUnordered::new();
        futures.push(task_rx_update_display_params);
        futures.push(task_rx);
        futures.push(task_forward_tx);

        while let Some(result) = futures.next().await {
            result?;
        }

        info!("WebSocket all tasks finished.");

        Ok::<(), JsError>(())
    };

    // Spawn this controller task on the JS event loop
    wasm_bindgen_futures::spawn_local(async move {
        let _cancel_tx_always_alive = _cancel_tx_always_alive;
        let cancel_fut = cancel_rx.next();

        futures::select! {
            res = task_main.fuse() => {
                if let Err(e) = res {
                    error!("WebSocket main task ended with error: {:?}", e);
                }
            },
            _ = cancel_fut.fuse() => {
                closed = true;
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

    // Wrapper that will tell us when the JS side has GC'ed the closure
    let mut cancel_on_drop_wrapped = OnDrop::new(
        || {
            debug!("Cancel closure dropped");
        },
        move || {
            // If this has already been called, do nothing.
            // TODO: This isn't actually reading the value of closed, since
            // TODO: it is copied into this closure.
            if !closed {
                debug!("Closing websocket connection");
                let _ = cancel_token_outer.send(());
            } else {
                warn!("Websocket connection already closed");
            }
        },
    );

    // Return a closure that when called, will cancel the connection.
    let cancel_closure = Closure::wrap(Box::new(move || {
        cancel_on_drop_wrapped();
    }) as Box<dyn FnMut()>);

    let update_display_params_closure =
        Closure::wrap(Box::new(move |params: JsDisplayParameters| {
            update_display_params_tx
                .unbounded_send(params)
                .map_err(|e| {
                    JsError::new(&format!(
                        "Failed to send updated display parameters: {:?}",
                        e
                    ))
                })
        })
            as Box<dyn FnMut(JsDisplayParameters) -> Result<(), JsError>>);

    let dispatchers = WsDispatchers {
        close_connection: cancel_closure.into_js_value().into(),
        update_display_parameters: update_display_params_closure.into_js_value().into(),
    };

    Ok(dispatchers)
}

#[wasm_bindgen(start)]
pub fn main() {
    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));
}
