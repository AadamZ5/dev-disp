use dev_disp_core::{
    client::ScreenReceiverAdapter,
    host::{DisplayParameters, ScreenContentParameters},
    util::PinnedLocalFuture,
};
use dev_disp_encoders::toolkit::decoder::{Decoder, DecoderAction, DecoderDirectAction};
use dev_disp_transports::websocket::receiver::WsReceiverPrepState;
use futures::{channel::mpsc, FutureExt, StreamExt};
use thiserror::Error;

pub struct WsDesktopAdapter<D> {
    display_parameters_rx: mpsc::Receiver<DisplayParameters>,
    buffer_write: triple_buffer::Input<Vec<u8>>,
    decoder: D,
}

impl<D> WsDesktopAdapter<D> {
    pub fn new(
        display_parameters_rx: mpsc::Receiver<DisplayParameters>,
        buffer_write: triple_buffer::Input<Vec<u8>>,
        decoder: D,
    ) -> Self
    where
        D: Decoder,
    {
        Self {
            display_parameters_rx,
            buffer_write,
            decoder,
        }
    }
}

#[derive(Debug, Error)]
pub enum WsDesktopAdapterError {
    #[error("Decoder error")]
    DecoderError,
    #[error("Internal channel closed")]
    ChannelClosed,
    #[error("Error while negotiating codec")]
    CodecNegotiationError,
}

impl<D> ScreenReceiverAdapter<WsReceiverPrepState> for WsDesktopAdapter<D>
where
    D: Decoder,
{
    type Error = WsDesktopAdapterError;

    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        async move { Ok(()) }.boxed_local()
    }

    fn on_loading_screen(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        async move { Ok(()) }.boxed_local()
    }

    fn provide_display_config(
        &mut self,
    ) -> PinnedLocalFuture<'_, Result<DisplayParameters, Self::Error>> {
        self.display_parameters_rx
            .next()
            .map(|p| p.ok_or(WsDesktopAdapterError::ChannelClosed))
            .boxed_local()
    }

    fn close(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        // TODO: DO ME
        async move { Ok(()) }.boxed_local()
    }

    fn prepare_receive_screen_data<'s>(
        &'s mut self,
        _parameters: &'s ScreenContentParameters,
        mut transport_data: WsReceiverPrepState,
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>> {
        async move {
            let possible = transport_data.get_possible_codec_options().iter().collect();

            let preferred = self
                .decoder
                .get_preferred_configurations(possible)
                .await
                .into_iter()
                .cloned()
                .collect();

            transport_data
                .send_preferred_codec_options(preferred)
                .await
                .map_err(|_| WsDesktopAdapterError::CodecNegotiationError)?;

            let final_codec = transport_data
                .get_final_codec()
                .await
                .map_err(|_| WsDesktopAdapterError::CodecNegotiationError)?;

            self.decoder
                .set_codec(&final_codec)
                .await
                .map_err(|_| WsDesktopAdapterError::DecoderError)?;

            transport_data
                .declare_prepared()
                .await
                .map_err(|_| WsDesktopAdapterError::CodecNegotiationError)?;

            Ok(())
        }
        .boxed_local()
    }

    fn receive_screen_data<'s>(
        &'s mut self,
        data: &'s [u8],
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>> {
        async move {
            let mut write_buffer = self.buffer_write.input_buffer_publisher();

            match self.decoder.decode_into(data, &mut write_buffer).await {
                DecoderDirectAction::Decoded => Ok(()),
                DecoderDirectAction::NoFrame => Ok(()),
                DecoderDirectAction::Error(_) => Err(WsDesktopAdapterError::DecoderError),
            }
        }
        .boxed_local()
    }
}
