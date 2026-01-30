use crate::{daemon::api::DevDispApi, util::PinnedFuture};

/// Represents an endpoint that can serve the DevDispDaemon API.
pub trait DevDispApiEndpoint {
    /// Given an API implementation, serve it on this endpoint.
    fn serve_api<D>(&mut self, api: D) -> PinnedFuture<'static, ()>
    where
        D: DevDispApi + Send + Sync + 'static;
}
