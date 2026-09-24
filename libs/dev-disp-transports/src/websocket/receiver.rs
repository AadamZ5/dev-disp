use bincode::error::{DecodeError, EncodeError};
use dev_disp_core::{
    client::{ScreenReceiverAdapter, ScreenTransportReceiver},
    coding::{
        encoder::CodecOption,
        messages::{CodecNegotiationClient, CodecNegotiationServer},
    },
    core::{DevDispMessageFromClient, DevDispMessageFromSource},
    util::PinnedLocalFuture,
};
use futures::{
    FutureExt, Sink, SinkExt, Stream, StreamExt,
    channel::mpsc,
    stream::{SplitSink, SplitStream},
};
use log::{debug, warn};
use thiserror::Error;

use crate::websocket::messages::{WsMessageFromClient, WsMessageFromSource};

pub struct WsReceiver<S> {
    tx: SplitSink<S, Vec<u8>>,
    rx: SplitStream<S>,
}

impl<'a, S> WsReceiver<S>
where
    S: Sink<Vec<u8>> + Stream<Item = Vec<u8>> + Unpin,
    S::Error: std::error::Error + 'static,
{
    pub fn new(duplex: S) -> Self
    where
        S: Sink<Vec<u8>> + Stream<Item = Vec<u8>> + Unpin,
    {
        let (tx, rx) = duplex.split();

        Self { tx, rx }
    }

    pub fn enc(m: WsMessageFromClient) -> Result<Vec<u8>, EncodeError> {
        bincode::serde::encode_to_vec(&m, bincode::config::standard())
    }

    pub fn dec(data: &[u8]) -> Result<(WsMessageFromSource<'_>, usize), DecodeError> {
        bincode::serde::borrow_decode_from_slice(data, bincode::config::standard())
    }

    pub async fn send(&mut self, msg: WsMessageFromClient) -> Result<(), WsReceiverError> {
        let encoded_msg =
            Self::enc(msg).map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
        self.tx
            .send(encoded_msg)
            .await
            .map_err(|e| WsReceiverError::SendError(Box::new(e)))
    }
}

pub struct WsReceiverPrepState {
    tx: mpsc::Sender<CodecNegotiationClient>,
    rx: mpsc::Receiver<CodecNegotiationServer>,
    possible_codec_options: Vec<CodecOption>,
}

impl WsReceiverPrepState {
    pub fn new(
        codec_options: Vec<CodecOption>,
        tx: mpsc::Sender<CodecNegotiationClient>,
        rx: mpsc::Receiver<CodecNegotiationServer>,
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
            .send(CodecNegotiationClient::ResponsePreferredEncodings(
                preferred_codecs,
            ))
            .await
            .map_err(|e| WsReceiverError::SendError(Box::new(e)))
    }

    pub async fn get_final_codec(&mut self) -> Result<CodecOption, WsReceiverError> {
        loop {
            match self.rx.next().await {
                Some(CodecNegotiationServer::RequestSetEncoding(codec)) => return Ok(codec),
                Some(_) => {
                    warn!("Unexpected message received while waiting for final codec");
                }
                None => {
                    return Err(WsReceiverError::UnexpectedEnd);
                }
            }
        }
    }

    pub async fn declare_prepared(&mut self) -> Result<(), WsReceiverError> {
        self.tx
            .send(CodecNegotiationClient::ResponseSetEncoding(true))
            .await
            .map_err(|e| WsReceiverError::SendError(Box::new(e)))
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
    TransportError(Box<dyn std::error::Error + 'static>),
    #[error("Send error: {0}")]
    SendError(Box<dyn std::error::Error + 'static>),
    #[error("Unexpected end of stream")]
    UnexpectedEnd,
}

impl<'a, S> ScreenTransportReceiver<WsReceiverPrepState> for WsReceiver<S>
where
    S: Sink<Vec<u8>> + Stream<Item = Vec<u8>> + Unpin,
    S::Error: std::error::Error + 'static,
{
    type Error = WsReceiverError;

    fn initialize<'s>(&'s mut self) -> PinnedLocalFuture<'s, Result<(), Self::Error>> {
        async { Ok(()) }.boxed_local()
    }

    fn listen<'s, T>(&'s mut self, mut adapter: T) -> PinnedLocalFuture<'s, Result<(), Self::Error>>
    where
        T: ScreenReceiverAdapter<WsReceiverPrepState> + 's,
        T::Error: std::error::Error + 'static,
    {
        // We use our rx/tx and the adapter to handle the protocol
        async move {
            while let Some(raw_msg) = self.rx.next().await {
                let msg = Self::dec(&raw_msg)
                    .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?
                    .0;

                // Handle the incoming message using the adapter
                match msg {
                    WsMessageFromSource::RequestPreInit => {
                        self.send(WsMessageFromClient::ResponsePreInit).await?;
                    }
                    WsMessageFromSource::RequestDeviceInformation => {
                        let display_config = adapter
                            .provide_display_config()
                            .await
                            .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                        self.send(WsMessageFromClient::ResponseDeviceInformation(
                            display_config.into(),
                        ))
                        .await?;
                    }
                    WsMessageFromSource::RequestProtocolInit(ws_message_protocol_init) => {
                        // TODO: Better initialization phase! Security!
                        self.send(WsMessageFromClient::ResponseProtocolInit(
                            ws_message_protocol_init,
                        ))
                        .await?;
                    }
                    WsMessageFromSource::Core(dev_disp_message_from_source) => {
                        match dev_disp_message_from_source {
                            DevDispMessageFromSource::GetDisplayParametersRequest => {
                                let display_config = adapter
                                    .provide_display_config()
                                    .await
                                    .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                                self.send(WsMessageFromClient::Core(
                                    DevDispMessageFromClient::DisplayParametersUpdate(
                                        display_config,
                                    ),
                                ))
                                .await?;
                            }
                            DevDispMessageFromSource::PutScreenData(items) => {
                                adapter
                                    .recieve_screen_data(&items)
                                    .await
                                    .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                            }
                        }
                    }
                    WsMessageFromSource::CodecNegotiation(
                        CodecNegotiationServer::RequestPreferredEncodings(
                            screen_content_parameters,
                            codec_options,
                        ),
                    ) => {
                        // This is fucking dumb. Because of E0276, we cannot properly attach our
                        // state as a GAT of the `ScreenTransportReceiver` trait, which means
                        // we can't properly specify lifetimes to borrow the websocket tx/rx handles,
                        // so we have to do a stupid stupid message forwarding handling here to facilitate
                        // the handshake instead of the state just taking the handles directly.
                        //
                        // So fucking stupid.
                        // https://github.com/rust-lang/rust/issues/134890
                        //
                        // The below is really ugly, but it will work for now.

                        let (ws_send_tx, mut ws_send_rx) =
                            mpsc::channel::<CodecNegotiationClient>(8);
                        let (mut ws_receive_tx, ws_receive_rx) =
                            mpsc::channel::<CodecNegotiationServer>(8);

                        let ws_tx = &mut self.tx;

                        let fw_outbound = async move {
                            while let Ok(mxg) = ws_send_rx.recv().await {
                                let mxg = WsMessageFromClient::CodecNegotiation(mxg);
                                let mxg = Self::enc(mxg)
                                    .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                                ws_tx
                                    .send(mxg)
                                    .await
                                    .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                            }

                            Ok::<(), WsReceiverError>(())
                        };

                        let ws_rx = &mut self.rx;

                        let fw_inbound = async move {
                            while let Some(mxg) = ws_rx.next().await {
                                let decoded_mxg = Self::dec(&mxg)
                                    .map_err(|e| WsReceiverError::TransportError(Box::new(e)))?
                                    .0;

                                let codec_negotiation_msg = match decoded_mxg {
                                    WsMessageFromSource::CodecNegotiation(msg) => msg,
                                    _ => {
                                        warn!(
                                            "Received unexpected message during codec negotiation"
                                        );
                                        continue;
                                    }
                                };

                                ws_receive_tx
                                    .send(codec_negotiation_msg)
                                    .await
                                    .map_err(|e| WsReceiverError::SendError(Box::new(e)))?;
                            }

                            Ok::<(), WsReceiverError>(())
                        };

                        let prep_state =
                            WsReceiverPrepState::new(codec_options, ws_send_tx, ws_receive_rx);
                        let negotiation = adapter
                            .prepare_receive_screen_data(&screen_content_parameters, prep_state);

                        // Join the negotiation and outbound tasks to make sure we drain all messages from the
                        // negotiation phase.
                        let outgoing = async move {
                          futures::join!(negotiation, fw_outbound)
                        };
                        
                        // Progress the outbound and inbound tasks concurrently, making sure that if the outbound
                        // is fully drained, we stop processing further inbound messages, and release
                        // back to the outer scope.
                        futures::select_biased! {
                            tuple = outgoing.fuse() => {
                                let (negotiation_rslt, fw_out_rslt) = tuple;
                                negotiation_rslt.map_err(|e| WsReceiverError::TransportError(Box::new(e)))?;
                                fw_out_rslt?;
                            }
                            fw_in_rslt = fw_inbound.fuse() => {
                                fw_in_rslt?;
                            }
                        }

                        // JFC that sucks.
                    }
                    WsMessageFromSource::CodecNegotiation(_) => {
                        warn!("Received CodecNegotiation message outside of negotiation phase");
                    }
                }
            }

            Ok(())
        }
        .boxed_local()
    }
}
