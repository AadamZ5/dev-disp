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
