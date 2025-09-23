use crate::client::DisplayHostInfo;

pub enum DevDispHostMessage<'a> {
    GetDisplayConfig,
    ScreenData(&'a [u8]),
}

pub enum DevDispClientMessage {
    UpdateDisplayConfig(DisplayHostInfo),
}
