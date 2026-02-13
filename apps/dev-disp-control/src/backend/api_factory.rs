use crate::backend::DisconnectableApi;
use dev_disp_core::{daemon::api::DevDispApi, util::PinnedFuture};
use futures::FutureExt;

/// Frontend trait that determines how to create a backend API instance.
/// This allows for custom setup and instantiation of the backend, allowing for
/// different backend implementations to be easily swapped in and out.
pub trait ApiFactory {
    type Api: DevDispApi + DisconnectableApi + Send + 'static;
    type ConnectParam: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send;

    /// Creates a new API instance, possibly reusing the last instance if one was available.
    fn create_api(
        &self,
        last_instance: Option<Self::Api>,
        param: Self::ConnectParam,
    ) -> PinnedFuture<'static, Result<Self::Api, Box<dyn std::error::Error + Send + Sync>>>;
}

/// A simple `ApiFactory` implementation that takes a callback function to create the API instance.
pub struct CallbackApiFactory<F, A, CP, Fut>
where
    F: Fn(Option<A>, CP) -> Fut,
    Fut: Future<Output = Result<A, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    A: DevDispApi + DisconnectableApi + 'static,
    CP: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send,
{
    factory_fn: F,
    _marker_api: std::marker::PhantomData<A>,
    _marker_param: std::marker::PhantomData<CP>,
}

impl<F, A, CP, Fut> CallbackApiFactory<F, A, CP, Fut>
where
    F: Fn(Option<A>, CP) -> Fut,
    Fut: Future<Output = Result<A, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
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

impl<F, A, CP, Fut> ApiFactory for CallbackApiFactory<F, A, CP, Fut>
where
    F: Fn(Option<A>, CP) -> Fut,
    Fut: Future<Output = Result<A, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    A: DevDispApi + DisconnectableApi + Send + 'static,
    CP: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send,
{
    type Api = A;
    type ConnectParam = CP;

    fn create_api(
        &self,
        last_instance: Option<Self::Api>,
        param: Self::ConnectParam,
    ) -> PinnedFuture<'static, Result<Self::Api, Box<dyn std::error::Error + Send + Sync>>> {
        (self.factory_fn)(last_instance, param).boxed()
    }
}

/// Helper function to create a callback factory from a future factory.
pub fn callback_api_factory<F, A, CP, Fut>(
    factory_fn: F,
) -> impl ApiFactory<Api = A, ConnectParam = CP>
where
    F: Fn(Option<A>, CP) -> Fut,
    Fut: Future<Output = Result<A, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    A: DevDispApi + DisconnectableApi + Send + 'static,
    CP: Clone + std::fmt::Debug + std::fmt::Display + 'static + Send,
{
    CallbackApiFactory::new(factory_fn)
}
