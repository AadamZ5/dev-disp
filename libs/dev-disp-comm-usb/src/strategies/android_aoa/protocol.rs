use bincode::{
    Decode, Encode,
    error::{DecodeError, EncodeError},
};

pub type MessageId = u16;

#[derive(Encode, Decode, Debug, Clone)]
pub struct Message<T> {
    pub id: MessageId,
    pub payload: T,
}

#[derive(Encode, Decode, Debug, Clone)]
pub enum MessageToAndroid {
    ScreenUpdate(Message<Vec<u8>>),
    GetScreenInfo(Message<()>),
    Quit(Message<()>),
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct ScreenInfo {
    pub width: u16,
    pub height: u16,
    pub bpp: u8,
    pub refresh_rate: u8,
}

#[derive(Encode, Decode, Debug, Clone)]
pub enum MessageFromAndroid {
    ScreenInfo(Message<ScreenInfo>),
    Ack(Message<u16>),
    Quit(Message<()>),
}

impl MessageToAndroid {
    pub fn id(&self) -> MessageId {
        match self {
            MessageToAndroid::ScreenUpdate(msg) => msg.id,
            MessageToAndroid::GetScreenInfo(msg) => msg.id,
            MessageToAndroid::Quit(msg) => msg.id,
        }
    }

    pub fn serialize(self) -> Result<Vec<u8>, EncodeError> {
        bincode::encode_to_vec(self, bincode::config::standard())
    }

    pub fn serialize_into(self, slice: &mut [u8]) -> Result<usize, EncodeError> {
        bincode::encode_into_slice(self, slice, bincode::config::standard())
    }

    pub fn deserialize(slice: &[u8]) -> Result<(MessageToAndroid, usize), DecodeError> {
        let (msg, size): (MessageToAndroid, _) =
            bincode::decode_from_slice(slice, bincode::config::standard())?;
        Ok((msg, size))
    }
}

impl MessageFromAndroid {
    pub fn id(&self) -> MessageId {
        match self {
            MessageFromAndroid::ScreenInfo(msg) => msg.id,
            MessageFromAndroid::Ack(msg) => msg.id,
            MessageFromAndroid::Quit(msg) => msg.id,
        }
    }

    pub fn serialize(self) -> Result<Vec<u8>, EncodeError> {
        bincode::encode_to_vec(self, bincode::config::standard())
    }

    pub fn serialize_into(self, slice: &mut [u8]) -> Result<usize, EncodeError> {
        bincode::encode_into_slice(self, slice, bincode::config::standard())
    }

    pub fn deserialize(slice: &[u8]) -> Result<(Self, usize), DecodeError> {
        let (msg, size): (MessageFromAndroid, _) =
            bincode::decode_from_slice(slice, bincode::config::standard())?;
        Ok((msg, size))
    }
}
