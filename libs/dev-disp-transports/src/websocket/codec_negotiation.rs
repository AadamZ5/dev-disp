use dev_disp_core::{
    coding::encoder::{
        Encoder, EncoderPossibleCodec, EncoderPossibleCodecInternal,
        map_external_to_internal_configs, map_internal_to_external_configs,
    },
    host::ScreenContentParameters,
    util::PinnedLocalFuture,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use log::warn;
use thiserror::Error;

use crate::websocket::messages::{WsMessageFromClient, WsMessageFromSource};

// TODO: Move to encoders crate

pub trait CodecChecker {
    fn check_codecs<'s>(
        &'s self,
        possible_codecs: &Vec<EncoderPossibleCodec>,
    ) -> PinnedLocalFuture<'s, Vec<EncoderPossibleCodec>>;
}

#[derive(Debug, Error)]
pub enum ClientNegotiationError {
    #[error("No compatible codecs found")]
    NoCompatibleCodecs {
        possible_codecs: Vec<EncoderPossibleCodec>,
    },
    #[error("No codecs were provided by the server")]
    NoCodecsProvided,
    #[error("Failed to send message to the server")]
    SendError(Box<dyn std::error::Error + 'static>),
    #[error("Negotiation ended before it was completed")]
    NegotiationIncomplete,
}

pub async fn negotiate_as_client<'a, Tx, Rx, C>(
    mut tx: Tx,
    mut rx: Rx,
    codec_checker: C,
) -> Result<EncoderPossibleCodec, ClientNegotiationError>
where
    Tx: Sink<WsMessageFromClient> + Unpin,
    Tx::Error: std::error::Error + 'static,
    Rx: Stream<Item = WsMessageFromSource<'a>> + Unpin,
    C: CodecChecker,
{
    let possible_codecs = loop {
        match rx.next().await {
            Some(WsMessageFromSource::RequestPreferredEncodings(possible_codecs)) => {
                break possible_codecs;
            }
            Some(msg) => {
                warn!(
                    "Unexpected message received from server during codec negotiation: {:?}",
                    msg
                );
            }
            None => return Err(ClientNegotiationError::NoCodecsProvided),
        }
    };

    let preferred_encodings = codec_checker.check_codecs(&possible_codecs).await;

    if preferred_encodings.is_empty() {
        return Err(ClientNegotiationError::NoCompatibleCodecs { possible_codecs });
    }

    tx.send(WsMessageFromClient::ResponsePreferredEncodings(
        preferred_encodings,
    ))
    .await
    .map_err(|e| ClientNegotiationError::SendError(Box::new(e)))?;

    let set_codec = loop {
        match rx.next().await {
            Some(WsMessageFromSource::SetEncoding(codec)) => {
                break codec;
            }
            Some(msg) => {
                warn!(
                    "Unexpected message received from server during codec negotiation: {:?}",
                    msg
                );
            }
            None => return Err(ClientNegotiationError::NegotiationIncomplete),
        }
    };

    Ok(set_codec)
}

pub trait CodecProvider {
    type CodecData;
    type Error;

    fn get_supported_codecs<'s>(
        &'s mut self,
        screen_content_parameters: &ScreenContentParameters,
    ) -> PinnedLocalFuture<
        's,
        Result<Vec<EncoderPossibleCodecInternal<Self::CodecData>>, Self::Error>,
    >;

    fn set_codec<'s, 'p>(
        &'s mut self,
        parameters: &'p ScreenContentParameters,
        preferred_encoders: Option<Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>>,
        offered_encoders: Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>,
    ) -> PinnedLocalFuture<'s, Result<&'p EncoderPossibleCodecInternal<Self::CodecData>, Self::Error>>
    where
        'p: 's;
}

impl<T> CodecProvider for T
where
    T: Encoder,
{
    type CodecData = T::CodecData;
    type Error = T::Error;

    fn get_supported_codecs<'s>(
        &'s mut self,
        screen_content_parameters: &ScreenContentParameters,
    ) -> PinnedLocalFuture<
        's,
        Result<Vec<EncoderPossibleCodecInternal<Self::CodecData>>, Self::Error>,
    > {
        self.get_supported_configurations(screen_content_parameters)
    }

    fn set_codec<'s, 'p>(
        &'s mut self,
        parameters: &'p ScreenContentParameters,
        preferred_encoders: Option<Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>>,
        offered_encoders: Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>,
    ) -> PinnedLocalFuture<'s, Result<&'p EncoderPossibleCodecInternal<Self::CodecData>, Self::Error>>
    where
        'p: 's,
    {
        self.set_codec(parameters, preferred_encoders, offered_encoders)
    }
}

#[derive(Debug, Error)]
pub enum ServerNegotiationError {
    #[error("The codec provider produced no possible codecs")]
    NoPossibleCodecs,
    #[error("Encoder provider failed to generate supported encodings")]
    EncoderProviderError(Box<dyn std::error::Error + 'static>),
    #[error("Failed to set codec")]
    SetCodecFailure(Option<EncoderPossibleCodec>),
    #[error("Failed to send message to the client")]
    SendError(Box<dyn std::error::Error + 'static>),
    #[error("Negotiation ended before it was completed")]
    NegotiationIncomplete,
}

pub async fn negotiate_as_server<'a, Tx, Rx, E>(
    screen_content_parameters: &ScreenContentParameters,
    mut tx: Tx,
    mut rx: Rx,
    codec_provider: &mut E,
) -> Result<EncoderPossibleCodec, ServerNegotiationError>
where
    Tx: Sink<WsMessageFromSource<'a>> + Unpin,
    Tx::Error: std::error::Error + 'static,
    Rx: Stream<Item = WsMessageFromClient> + Unpin,
    E: CodecProvider,
    E::Error: std::error::Error + 'static,
{
    let possible_codecs = codec_provider
        .get_supported_codecs(screen_content_parameters)
        .await
        .map_err(|e| ServerNegotiationError::EncoderProviderError(Box::new(e)))?;

    if possible_codecs.is_empty() {
        return Err(ServerNegotiationError::NoPossibleCodecs);
    }

    let (external_codecs, internal_map) = map_internal_to_external_configs(possible_codecs);

    tx.send(WsMessageFromSource::RequestPreferredEncodings(
        external_codecs,
    ))
    .await
    .map_err(|e| ServerNegotiationError::SendError(Box::new(e)))?;

    let preferred_encodings = loop {
        match rx.next().await {
            Some(WsMessageFromClient::ResponsePreferredEncodings(codecs)) => {
                break codecs;
            }
            Some(msg) => {
                warn!(
                    "Unexpected message received from client during codec negotiation: {:?}",
                    msg
                );
            }
            None => return Err(ServerNegotiationError::NegotiationIncomplete),
        }
    };

    let preferred_codecs_internal =
        map_external_to_internal_configs(preferred_encodings, &internal_map).collect::<Vec<_>>();

    let all_codecs_internal = internal_map.values().collect::<Vec<_>>();

    let set_codec = codec_provider
        .set_codec(
            screen_content_parameters,
            Some(preferred_codecs_internal),
            all_codecs_internal,
        )
        .await
        .map_err(|_| ServerNegotiationError::SetCodecFailure(None))?;

    tx.send(WsMessageFromSource::SetEncoding(set_codec.for_send(0)))
        .await
        .map_err(|e| ServerNegotiationError::SendError(Box::new(e)))?;

    Ok(set_codec.for_send(0))
}
