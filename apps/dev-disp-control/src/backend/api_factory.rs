use crate::backend::DisconnectableApi;
use dev_disp_core::{daemon::api::DevDispApi, util::PinnedFuture};

pub trait ApiFactory {
    type Api: DevDispApi + DisconnectableApi + 'static;
    type ConnectParam: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send;

    /// Creates a new API instance, possibly reusing the last instance if one was available.
    fn create_api(
        &self,
        last_instance: Option<Self::Api>,
        param: Self::ConnectParam,
    ) -> PinnedFuture<'static, Result<Self::Api, Box<dyn std::error::Error + Send + Sync>>>;
}

pub struct CallbackApiFactory<F, A, CP>
where
    F: Fn(
            Option<A>,
            CP,
        ) -> PinnedFuture<'static, Result<A, Box<dyn std::error::Error + Send + Sync>>>
        + Send
        + Sync
        + 'static,
    A: DevDispApi + DisconnectableApi + 'static,
    CP: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send,
{
    factory_fn: F,
    _marker_api: std::marker::PhantomData<A>,
    _marker_param: std::marker::PhantomData<CP>,
}

impl<F, A, CP> CallbackApiFactory<F, A, CP>
where
    F: Fn(
            Option<A>,
            CP,
        ) -> PinnedFuture<'static, Result<A, Box<dyn std::error::Error + Send + Sync>>>
        + Send
        + Sync
        + 'static,
    A: DevDispApi + DisconnectableApi + 'static,
    CP: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send,
{
    pub fn new(factory_fn: F) -> Self {
        Self {
            factory_fn,
            _marker_api: std::marker::PhantomData,
            _marker_param: std::marker::PhantomData,
        }
    }
}

impl<F, A, CP> ApiFactory for CallbackApiFactory<F, A, CP>
where
    F: Fn(
            Option<A>,
            CP,
        ) -> PinnedFuture<'static, Result<A, Box<dyn std::error::Error + Send + Sync>>>
        + Send
        + Sync
        + 'static,
    A: DevDispApi + DisconnectableApi + 'static,
    CP: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send,
{
    type Api = A;
    type ConnectParam = CP;

    fn create_api(
        &self,
        last_instance: Option<Self::Api>,
        param: Self::ConnectParam,
    ) -> PinnedFuture<'static, Result<Self::Api, Box<dyn std::error::Error + Send + Sync>>> {
        (self.factory_fn)(last_instance, param)
    }
}
