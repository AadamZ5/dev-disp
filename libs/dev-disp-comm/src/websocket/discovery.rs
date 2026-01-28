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
    transport::WsTransport,
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
            if let Err(e) = self.take_ws_tx.send(get_ws_tx).await {
                error!("Error requesting to takeover connection: {}", e);
            }
            let websocket = match get_ws_rx.await {
                Err(e) => {
                    error!("Error waiting for connection to be handed to us: {}", e);
                    return Err(Box::new(e) as Box<dyn Error + Send + Sync>);
                }
                Ok(ws) => ws,
            };

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
    connections_update_notification: mpsc::Receiver<()>,
}

impl<S> Default for WsDiscovery<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> WsDiscovery<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new() -> Self {
        let (connections_update_tx, connections_update_rx) = mpsc::channel(100);
        let current_connections = Arc::new(RwLock::new(HashMap::new()));
        Self {
            current_connections: current_connections.clone(),
            listen_ctx: WsDiscoveryListenCtx {
                current_connections,
                connections_update_tx,
            },
            connections_update_notification: connections_update_rx,
        }
    }

    /// Listen for incoming WebSocket connections from devices.
    ///
    /// The provided stream should yield accepted TCP streams that
    /// are ready to be upgraded to WebSocket connections.
    ///
    /// The resulting future should be run as it's own "background" task.
    /// Without running this future, the discovery will not function.
    pub fn listen<'a, I>(
        &self,
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
            let (mut connection_task_tx, mut connection_task_rx) =
                mpsc::channel::<Pin<Box<dyn Future<Output = ()>>>>(10);

            // With this task set, we will:
            // - Loop and accept incoming connections
            // - For each incoming connection, spawn a new task to do the pre-initialization
            //   handshake, and then register the device if successful.
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
                    if let Err(e) = connection_task_tx.send(init_task).await {
                        error!("Error sending new connection for spawning: {}", e);
                    }
                }
            };

            tasks.push(incoming_connections_task.boxed_local());

            loop {
                futures::select! {
                    _ = tasks.next() => {
                        // Tasks should only be empty if our incoming connections loop has
                        // finished, which means we should expect no more incomming connections
                        // and we can break this loop.
                        if tasks.is_empty() {
                            break;
                        }
                    },
                    new_task = connection_task_rx.next() => {
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

    /// Handles pre-initialization handshake for a new WebSocket connection.
    ///
    /// This function performs the necessary handshake to verify and gather
    /// information about the connecting device before registering it for discovery, and
    /// ensure we're talking to a client that follows the expected protocol.
    ///
    /// The returned future will live as long as the device is connected and not yet claimed.
    ///
    /// TODO: Do sanity checks for if the device is still connected while waiting to be claimed,
    /// and remove the device if it disconnects!
    async fn pre_init(listen_ctx: &WsDiscoveryListenCtx<S>, mut ws_stream: WebSocketStream<S>) {
        // First talk to the websocket using the pre-init messages to figure
        // out details about the connecting device.

        // Do pre-init sanity check
        info!("Starting WebSocket pre-init handshake...");
        let pre_init_req = WsMessageFromSource::RequestPreInit;
        let pre_init_req_bytes =
            match bincode::serde::encode_to_vec(&pre_init_req, bincode::config::standard()) {
                Ok(vec) => vec,
                Err(e) => {
                    error!("Failed to encode pre-init request: {}", e);
                    return;
                }
            };

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
        let device_info_req_bytes =
            match bincode::serde::encode_to_vec(&device_info_req, bincode::config::standard()) {
                Ok(vec) => vec,
                Err(e) => {
                    error!("Failed to encode device info request: {}", e);
                    return;
                }
            };

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

        // TODO: Implement a listener of incoming data, so we can detect when the incoming
        // data stream closes, and the device can be assumed disconnected while waiting
        // to be claimed.

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

    fn get_display_name(&self) -> String {
        "WebSocket".to_string()
    }
}

impl<S> StreamingDeviceDiscovery for WsDiscovery<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = Vec<Self::DeviceCandidate>> + Send>> {
        Box::pin(futures::stream::unfold(self, |mut this| async move {
            let notification = this.connections_update_notification.next().await;
            notification?;
            Some((this.discover_devices().await, this))
        }))
    }
}
