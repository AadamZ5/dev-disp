use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::api::DevDispApiFacade;

#[derive(Serialize, Deserialize)]
pub struct InitializeDeviceParams {
    pub discovery_id: String,
    pub device_id: String,
}

#[derive(Serialize, Deserialize)]
pub enum JsonApiCommand {
    GetDevices,
    InitializeDevice(),
}

struct JsonOverPipeApi<F, P>
where
    F: DevDispApiFacade,
    P: AsyncWrite + AsyncRead + Unpin,
{
    pipe: P,
    facade: F,
}

impl<F, P> JsonOverPipeApi<F, P>
where
    F: DevDispApiFacade,
    P: AsyncWrite + AsyncRead + Unpin,
{
    pub fn new(pipe: P, facade: F) -> Self {
        Self { pipe, facade }
    }

    pub async fn listen(&mut self) {}
}
