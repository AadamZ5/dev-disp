use dev_disp_core::{
    client::ScreenReceiverAdapter, coding::encoder::CodecOption, host::DisplayParameters,
    util::PinnedLocalFuture,
};
use dev_disp_transports::websocket::receiver::WsReceiverPrepState;
use futures::FutureExt;
use js_sys::{SharedArrayBuffer, Uint8Array};
use log::{debug, warn};
use serde_wasm_bindgen::Error as SerdeWasmBindgenError;
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::types::{DevDispEvent, JsDisplayParameters, JsEncoderPossibleConfiguration, WsHandlers};

/// This JS adapter is essentially the connector to our JS application.
/// The [dev_disp_transports::websocket::transport::WsListener] will be able to use
/// this adapter to command our app based on the incoming messages. All we must do
/// is adapt these functions to implement [ScreenReceiverAdapter].
///
/// Note that we don't correctly implement a [Decoder] right now, since the
/// webpage API is actually doing the decoding. We should however implement
/// that since there is technically a decoder API we are dealing with.
#[derive(Debug, Clone)]
pub struct JsAdapter {
    /// The WebSocket handlers for this adapter
    handlers: WsHandlers,
    /// The underlying buffer for screen data
    buffer: Uint8Array,
    /// If we have a shared array buffer
    have_sab: bool,
}

impl JsAdapter {
    pub fn new(handlers: WsHandlers, shared_buffer: Option<SharedArrayBuffer>) -> Self {
        Self {
            handlers,
            buffer: shared_buffer
                .as_ref()
                .map(|sab| Uint8Array::new(sab))
                .unwrap_or_else(|| Uint8Array::new_with_length(512 * 1024 * 1024)), // TODO: How much memory to allocate
            have_sab: shared_buffer.is_some(),
        }
    }
}

#[derive(Debug, Error)]
pub enum JsAdapterError {
    #[error("Unable to convert between WASM <-> JS type: {0}")]
    BindgenError(SerdeWasmBindgenError),
    #[error("Handler error: {0:?}")]
    HandlerError(JsValue),
    #[error("Error while negotiating codec")]
    CodecNegotiationError,
}

impl<'a> ScreenReceiverAdapter<WsReceiverPrepState> for JsAdapter {
    type Error = JsAdapterError;

    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        async move {
            if let Some(func) = &self.handlers.on_pre_init {
                let event = DevDispEvent {
                    error: None,
                    data: None,
                };
                let _ = func.call1(&JsValue::NULL, &event.into());
            }
            Ok(())
        }
        .boxed_local()
    }

    fn on_loading_screen(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        async move { Ok(()) }.boxed_local()
    }

    fn provide_display_config(
        &mut self,
    ) -> PinnedLocalFuture<'_, Result<dev_disp_core::host::DisplayParameters, Self::Error>> {
        async move {
            debug!("Handling GetDisplayParametersRequest message");
            let event = DevDispEvent {
                error: None,
                data: None,
            };
            let js_value = self
                .handlers
                .handle_request_display_parameters
                .call1(&JsValue::NULL, &event.into())
                .map_err(JsAdapterError::HandlerError)?;

            debug!("Got display parameters from handler: {:?}", js_value);

            let params = serde_wasm_bindgen::from_value::<JsDisplayParameters>(js_value)
                .map_err(JsAdapterError::BindgenError)?;

            let real_params: DisplayParameters = params.into();
            Ok(real_params)
        }
        .boxed_local()
    }

    fn close(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        todo!()
    }

    fn prepare_receive_screen_data<'s>(
        &'s mut self,
        parameters: &'s dev_disp_core::host::ScreenContentParameters,
        mut transport_data: WsReceiverPrepState,
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>> {
        async move {
            debug!(
                "Preparing to receive screen data with parameters: {:?}",
                parameters
            );

            let codec_options = transport_data.get_possible_codec_options();

            let js_codec_options = codec_options
                .iter()
                .cloned()
                .filter_map(|option| {
                    let js_option: JsEncoderPossibleConfiguration = option.into();
                    match serde_wasm_bindgen::to_value(&js_option) {
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

            let js_value_preferred_codec_options = self
                .handlers
                .handle_request_preferred_encoding
                .call1(&JsValue::NULL, &js_codec_options.into())
                .and_then(|handler_result| {
                    // We expect the handler result to be a JavaScript Promise.
                    handler_result.dyn_into::<js_sys::Promise>()
                })
                .map(|promise| JsFuture::from(promise))
                .map_err(JsAdapterError::HandlerError)?
                .await
                .map_err(JsAdapterError::HandlerError)?;

            let preferred_codec_options: Vec<CodecOption> =
                serde_wasm_bindgen::from_value::<Vec<JsEncoderPossibleConfiguration>>(
                    js_value_preferred_codec_options,
                )
                .map_err(JsAdapterError::BindgenError)?
                .into_iter()
                .map(|js_option| js_option.into())
                .collect::<Vec<CodecOption>>();

            transport_data
                .send_preferred_codec_options(preferred_codec_options)
                .await
                .map_err(|_| JsAdapterError::CodecNegotiationError)?;

            let final_codec = transport_data
                .get_final_codec()
                .await
                .map_err(|_| JsAdapterError::CodecNegotiationError)?;

            debug!("Final codec selected: {:?}", final_codec);

            let js_final_codec: JsEncoderPossibleConfiguration = final_codec.into();
            let js_value_final_codec = serde_wasm_bindgen::to_value(&js_final_codec)
                .map_err(JsAdapterError::BindgenError)?;

            self.handlers
                .handle_set_encoding
                .call1(&JsValue::NULL, &js_value_final_codec)
                .map_err(JsAdapterError::HandlerError)?;

            debug!("Set final codec: {:?}", js_final_codec);

            transport_data
                .declare_prepared()
                .await
                .map_err(|_| JsAdapterError::CodecNegotiationError)?;

            Ok(())
        }
        .boxed_local()
    }

    fn recieve_screen_data<'s>(
        &'s mut self,
        data: &'s [u8],
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>> {
        async move {
            let sub_buffer = self.buffer.subarray(0, data.len() as u32);
            sub_buffer.copy_from(data);

            // If we have a shared buffer, we don't need to copy the data to JS land.
            let event_data = if self.have_sab {
                // Send the length of the screen data, so they can collect it from the
                // shared array buffer.
                JsValue::from(sub_buffer.length())
            } else {
                // Send the uint8array directly
                JsValue::from(sub_buffer)
            };

            let event = DevDispEvent {
                error: None,
                data: Some(event_data),
            };

            self.handlers
                .handle_screen_data
                .call1(&JsValue::NULL, &event.into())
                .map_err(JsAdapterError::HandlerError)?;

            Ok(())
        }
        .boxed_local()
    }
}
