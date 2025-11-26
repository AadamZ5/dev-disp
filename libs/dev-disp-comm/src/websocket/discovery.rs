use std::{collections::HashMap, error::Error, pin::Pin, sync::Arc};

use async_tungstenite::{WebSocketStream, tungstenite::Message};
use dev_disp_core::{
    client::DisplayHost,
    host::{ConnectableDevice, ConnectableDeviceInfo, DeviceDiscovery, StreamingDeviceDiscovery},
    util::{PinnedFuture, PinnedLocalFuture},
};
use futures::{
    SinkExt,
    channel::{mpsc, oneshot},
    stream::FuturesUnordered,
};
use futures_locks::RwLock;
use futures_util::{AsyncRead, AsyncWrite, FutureExt, Stream, StreamExt};
use log::{debug, error, info, warn};
use uuid::Uuid;

use crate::websocket::{
    messages::{WsMessageFromClient, WsMessageFromSource},
    ws_transport::WsTransport,
};

#[derive(Debug)]
pub struct WsDeviceCandidate<S> {
    take_ws_tx: mpsc::Sender<oneshot::Sender<WebSocketStream<S>>>,
    device_info: ConnectableDeviceInfo,
}

impl<S> WsDeviceCandidate<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(
        take_ws_tx: mpsc::Sender<oneshot::Sender<WebSocketStream<S>>>,
        device_info: ConnectableDeviceInfo,
    ) -> Self {
        Self {
            take_ws_tx,
            device_info,
        }
    }
}

impl<S> Clone for WsDeviceCandidate<S> {
    fn clone(&self) -> Self {
        Self {
            take_ws_tx: self.take_ws_tx.clone(),
            device_info: self.device_info.clone(),
        }
    }
}

impl<S> ConnectableDevice for WsDeviceCandidate<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Transport = WsTransport<S>;

    fn connect(
        mut self,
    ) -> PinnedFuture<'static, Result<DisplayHost<Self::Transport>, Box<dyn Error + Send + Sync>>>
    {
        async move {
            let (get_ws_tx, get_ws_rx) = oneshot::channel();
            self.take_ws_tx.send(get_ws_tx).await.unwrap();
            let websocket = get_ws_rx.await.unwrap();
            Ok(DisplayHost::new(
                0,
                self.device_info.name,
                WsTransport::new(websocket),
            ))
        }
        .boxed()
    }

    fn get_info(&self) -> ConnectableDeviceInfo {
        self.device_info.clone()
    }
}

type CurrentConnections<S> = Arc<RwLock<HashMap<String, WsDeviceCandidate<S>>>>;

#[derive(Debug)]
struct WsDiscoveryListenCtx<S> {
    current_connections: CurrentConnections<S>,
    connections_update_tx: mpsc::Sender<()>,
}

impl<S> Clone for WsDiscoveryListenCtx<S> {
    fn clone(&self) -> Self {
        Self {
            current_connections: self.current_connections.clone(),
            connections_update_tx: self.connections_update_tx.clone(),
        }
    }
}

/// WebSocket-based device discovery.
///
/// Any incoming connections will be initialized, and once the sanity
/// handshake checks are done, they will be listed as connectable devices.
///
/// Once a device is chosen, it will be removed from the list of available devices.
pub struct WsDiscovery<S> {
    current_connections: Arc<RwLock<HashMap<String, WsDeviceCandidate<S>>>>,
    listen_ctx: WsDiscoveryListenCtx<S>,
    new_connection_notification: mpsc::Receiver<()>,
}

impl<S> WsDiscovery<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new() -> Self {
        let (new_connection_tx, new_connection_rx) = mpsc::channel(100);
        let current_connections = Arc::new(RwLock::new(HashMap::new()));
        Self {
            current_connections: current_connections.clone(),
            listen_ctx: WsDiscoveryListenCtx {
                current_connections: current_connections,
                connections_update_tx: new_connection_tx,
            },
            new_connection_notification: new_connection_rx,
        }
    }

    pub fn listen<'s, 'a, I>(
        &'s self,
        mut incoming_connections: I,
    ) -> PinnedLocalFuture<'a, Result<(), String>>
    where
        I: Stream<Item = S> + Unpin + Send + 'static,
    {
        let listen_ctx = self.listen_ctx.clone();

        async move {
            let listen_ctx = listen_ctx;

            // These channels will be used to transfer a *new* future that is created
            // when a new connection comes in, to the main task loop.
            let (mut new_connection_tx, mut new_connection_rx) =
                mpsc::channel::<Pin<Box<dyn Future<Output = ()>>>>(10);
            let mut tasks = FuturesUnordered::<Pin<Box<dyn Future<Output = ()>>>>::new();

            let listen_ctx_ref = &listen_ctx;

            let incoming_connections_task = async move {
                while let Some(incoming) = incoming_connections.next().await {
                    let ws_stream = match async_tungstenite::accept_async(incoming).await {
                        Ok(ws) => ws,
                        Err(e) => {
                            error!("WebSocket accept error: {}", e);
                            continue;
                        }
                    };

                    debug!("New WebSocket connection accepted.");

                    let init_task = Self::pre_init(listen_ctx_ref, ws_stream).boxed_local();
                    new_connection_tx.send(init_task).await.unwrap();
                }
            };

            tasks.push(incoming_connections_task.boxed_local());

            loop {
                futures::select! {
                    _ = tasks.next() => {
                        if tasks.is_empty() {
                            break;
                        }
                    },
                    new_task = new_connection_rx.next() => {
                        if let Some(task) = new_task {
                            tasks.push(task);
                        }
                    }
                }
            }

            info!("WebSocket discovery listener finished.");

            Ok(())
        }
        .boxed_local()
    }

    async fn pre_init(
        listen_ctx: &WsDiscoveryListenCtx<S>,
        mut ws_stream: WebSocketStream<S>,
    ) -> () {
        // First talk to the websocket using the pre-init messages to figure
        // out details about the connecting device.

        // Do pre-init sanity check
        info!("Starting WebSocket pre-init handshake...");
        let pre_init_req = WsMessageFromSource::RequestPreInit;
        let pre_init_req_bytes_result =
            bincode::serde::encode_to_vec(&pre_init_req, bincode::config::standard());
        if let Err(e) = pre_init_req_bytes_result {
            error!("Failed to encode pre-init request: {}", e);
            return;
        }
        let pre_init_req_bytes = pre_init_req_bytes_result.unwrap();

        info!("Sending pre-init request...");
        debug!("Pre-init request bytes: {:?}", pre_init_req_bytes);

        if let Err(e) = ws_stream.send(Message::binary(pre_init_req_bytes)).await {
            error!("Failed to send pre-init request: {}", e);
            return;
        }

        info!("Waiting for pre-init response...");

        let res = ws_stream.next().await.and_then(|msg| match msg {
            Ok(Message::Binary(bin)) => {
                let decoded: Result<(WsMessageFromClient, _), _> =
                    bincode::serde::decode_from_slice(&bin, bincode::config::standard());
                match decoded {
                    Ok((msg, _)) => Some(msg),
                    Err(e) => {
                        error!("Failed to decode pre-init response: {}", e);
                        None
                    }
                }
            }
            Ok(other) => {
                error!(
                    "Unexpected WebSocket message type during pre-init: {:?}",
                    other
                );
                None
            }
            Err(e) => {
                error!("WebSocket error during pre-init: {}", e);
                None
            }
        });

        if res.is_none() {
            error!("Did not receive valid pre-init response.");
            return;
        }

        info!("Pre-init response received.");

        info!("Requesting device info...");

        // Now we do device info
        let device_info_req = WsMessageFromSource::RequestDeviceInformation;
        let device_info_req_bytes_result =
            bincode::serde::encode_to_vec(&device_info_req, bincode::config::standard());
        if let Err(e) = device_info_req_bytes_result {
            error!("Failed to encode device info request: {}", e);
            return;
        }
        let device_info_req_bytes = device_info_req_bytes_result.unwrap();
        debug!("Device info request bytes: {:?}", device_info_req_bytes);

        if let Err(e) = ws_stream.send(Message::binary(device_info_req_bytes)).await {
            error!("Failed to send device info request: {}", e);
            return;
        }

        info!("Waiting for device info response...");

        let res = ws_stream.next().await.and_then(|msg| match msg {
            Ok(Message::Binary(bin)) => {
                let decoded: Result<(WsMessageFromClient, _), _> =
                    bincode::serde::decode_from_slice(&bin, bincode::config::standard());
                match decoded {
                    Ok((msg, _)) => Some(msg),
                    Err(e) => {
                        error!("Failed to decode device info response: {}", e);
                        None
                    }
                }
            }
            Ok(other) => {
                error!(
                    "Unexpected WebSocket message type during device info: {:?}",
                    other
                );
                None
            }
            Err(e) => {
                error!("WebSocket error during device info: {}", e);
                None
            }
        });

        let dev_info = match res {
            Some(WsMessageFromClient::ResponseDeviceInformation(info)) => info,
            _ => {
                error!("Did not receive valid device info response.");
                return;
            }
        };

        let id = Uuid::new_v4().to_string();

        info!("Registering device with id {}", &id);

        let (take_ws_tx, mut take_ws_rx) = mpsc::channel::<oneshot::Sender<WebSocketStream<S>>>(1);

        let device_info = ConnectableDeviceInfo {
            id: id.clone(),
            device_type: "WebSocket".to_string(),
            name: format!("WebSocket Device {}", dev_info.name),
            description: Some("A device connected via WebSocket".to_string()),
        };

        info!("Device info received: {:?}", device_info);

        let device_candidate = WsDeviceCandidate::new(take_ws_tx, device_info);

        listen_ctx
            .current_connections
            .write()
            .await
            .insert(id.clone(), device_candidate);

        // Notify about the new connection
        let mut devices_update_tx = listen_ctx.connections_update_tx.clone();
        let _ = devices_update_tx.try_send(());

        // Wait for someone to take the WebSocket
        if let Some(get_ws_tx) = take_ws_rx.next().await {
            debug!("Taking WebSocket connection for {}...", &id);
            listen_ctx.current_connections.write().await.remove(&id);
            let _ = get_ws_tx.send(ws_stream);
            let _ = devices_update_tx.try_send(());
            debug!("WebSocket connection for {} taken.", &id);
        } else {
            warn!("No one took the WebSocket connection from {}", id);
        }
    }
}

impl<S> DeviceDiscovery for WsDiscovery<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type DeviceCandidate = WsDeviceCandidate<S>;

    fn discover_devices(&self) -> PinnedFuture<'_, Vec<Self::DeviceCandidate>> {
        async move {
            let connections = self.current_connections.read().await;
            connections.values().cloned().collect()
        }
        .boxed()
    }
}

impl<S> StreamingDeviceDiscovery for WsDiscovery<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceCandidate>> + Send>> {
        Box::pin(futures::stream::unfold(self, |mut this| async move {
            let notification = this.new_connection_notification.next().await;
            if notification.is_none() {
                return None;
            }
            Some((this.discover_devices().await, this))
        }))
    }
}
