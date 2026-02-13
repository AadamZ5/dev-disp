use dev_disp_core::daemon::api::{DisplayHostStatus, InitializationState};
use futures::{FutureExt, Stream, StreamExt};

pub trait UnwrapOrLogMsg<T> {
    fn unwrap_or_log_msg(self, msg: &str) -> Option<T>;
}

pub trait UnwrapOrLogError<T> {
    fn unwrap_or_log_error(self) -> Option<T>;
}

pub trait UnwrapOrLog<T> {
    fn unwrap_or_log(self, msg: &str) -> Option<T>;
}

impl<T, E> UnwrapOrLog<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn unwrap_or_log(self, msg: &str) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(e) => {
                log::error!("{}: {}", msg, e);
                None
            }
        }
    }
}

impl<T, E> UnwrapOrLogMsg<T> for Result<T, E> {
    fn unwrap_or_log_msg(self, msg: &str) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(_) => {
                log::error!("{}", msg);
                None
            }
        }
    }
}

impl<T, E> UnwrapOrLogError<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn unwrap_or_log_error(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(e) => {
                log::error!("Error: {}", e);
                None
            }
        }
    }
}

pub fn status_to_display_string(status: &DisplayHostStatus) -> String {
    match status {
        DisplayHostStatus::Available => "Available".to_string(),
        DisplayHostStatus::InUse => "In Use".to_string(),
        DisplayHostStatus::Disconnecting => "Disconnecting".to_string(),
        DisplayHostStatus::Error(err) => format!("Error: {}", err),
        DisplayHostStatus::Initializing(phase) => {
            let phase_display_str = match phase {
                InitializationState::Unknown => "Unknown",
                InitializationState::Initializing => "Beginning initialization",
                InitializationState::InitializingTransport => "Initializing transport",
                InitializationState::GettingDisplayParameters => {
                    "Getting display parameters from client"
                }
                InitializationState::NotifyClientLoading => {
                    "Notifying client of loading virtual screen"
                }
                InitializationState::GettingScreen => "Preparing virtual screen",
                InitializationState::GettingEncoder => "Preparing encoder",
                InitializationState::NegotiatingCodecs => "Negotiating codecs with client",
                InitializationState::InitializingEncoder => "Initializing encoder",
                InitializationState::SettingClientCodec => "Setting client codec",
            };

            format!("Initializing: {}", phase_display_str)
        }
        DisplayHostStatus::Unknown => "Unknown".to_string(),
    }
}

pub trait MyStreamExt: Stream {
    /// Transforms a stream of any type into a stream that discards all items and outputs nothing.
    /// This can be useful when you want to execute a stream for its side effects but don't care about the output.
    /// This also erases the type, to allow this stream to be combined with others.
    fn no_output<T>(self) -> impl Stream<Item = T>
    where
        Self: Sized + Unpin,
    {
        async move {
            let mut stream = self;
            while let Some(_item) = stream.next().await {
                // Just consume the items and do nothing with them
            }

            // Return an empty stream of T
            futures::stream::empty()
        }
        .flatten_stream()
    }
}

impl<T> MyStreamExt for T where T: Stream {}
