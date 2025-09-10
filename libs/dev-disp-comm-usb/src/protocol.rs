use bincode::{BorrowDecode, Encode};

pub type MessageId = u16;

#[derive(Encode, BorrowDecode, Debug)]
pub enum EMessageOut<'a> {
    ScreenUpdate(&'a [u8]),
    GetScreenInfo,
    Quit,
}

#[derive(Encode, BorrowDecode, Debug)]
pub struct MessageOut<'a> {
    pub msg: EMessageOut<'a>,
    pub id: MessageId,
}

impl<'a> MessageOut<'a> {
    pub fn get_screen_info(req_id: u16) -> Self {
        Self {
            msg: EMessageOut::GetScreenInfo,
            id: req_id,
        }
    }

    pub fn screen_update(req_id: u16, data: &'a [u8]) -> Self {
        Self {
            msg: EMessageOut::ScreenUpdate(data),
            id: req_id,
        }
    }

    pub fn quit() -> Self {
        Self {
            msg: EMessageOut::Quit,
            id: 0,
        }
    }
}

#[derive(Encode, BorrowDecode, Debug)]
pub enum EMessageIn {
    ScreenInfo,
    Ack(u16),
    Quit,
}

#[derive(Encode, BorrowDecode, Debug)]
pub struct MessageIn {
    pub msg: EMessageIn,
    pub id: MessageId,
}

impl MessageIn {
    pub fn screen_info(for_id: u16) -> Self {
        Self {
            msg: EMessageIn::ScreenInfo,
            id: for_id,
        }
    }

    pub fn ack(for_id: u16) -> Self {
        Self {
            msg: EMessageIn::Ack(for_id),
            id: for_id,
        }
    }

    pub fn quit() -> Self {
        Self {
            msg: EMessageIn::Quit,
            id: 0,
        }
    }
}

pub fn serialize_out(msg: &MessageOut) -> Result<Vec<u8>, bincode::error::EncodeError> {
    todo!();
    //bincode::encode_into_writer(msg, bincode::config::standard())
}
