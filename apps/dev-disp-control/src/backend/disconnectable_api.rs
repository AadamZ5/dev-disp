use dev_disp_core::util::PinnedFuture;

pub trait DisconnectableApi {
    type ConnectParam: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send;

    /// When called, should return a future that resolves when
    /// the backend has disconnected. The future should resolve to an error if the disconnection
    /// was unexpected (e.g. connection lost, async task died, etc). If the disconnection
    /// was expected, then the future should resolve to `Ok(())`.
    fn on_disconnect(
        &self,
    ) -> PinnedFuture<'static, Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>>;

    fn disconnect(
        &mut self,
    ) -> PinnedFuture<'static, Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>>;
}
