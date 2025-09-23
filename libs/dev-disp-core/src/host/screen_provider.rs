use crate::client::{DisplayHost, ScreenTransport};

pub type DisplayHostResult<T> = Result<DisplayHost<T>, String>;

/// A screen provider is something that provides screen data to be
/// sent to some device
pub trait ScreenProvider {
    fn handle_display_host<T>(
        &self,
        host: DisplayHost<T>,
    ) -> impl Future<Output = DisplayHostResult<T>>
    where
        T: ScreenTransport + 'static;
}
