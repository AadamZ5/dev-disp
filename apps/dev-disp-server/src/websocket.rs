use dev_disp_comm::websocket::discovery::WsDiscovery;
use dev_disp_core::util::{PinnedLocalFuture, PinnedStream};
use futures_util::{FutureExt, StreamExt, stream};
use log::{error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

pub async fn create_tcp_client_stream() -> PinnedStream<'static, Compat<TcpStream>> {
    let listener = TcpListener::bind("0.0.0.0:56789")
        .await
        .expect("Failed to bind to TCP port 56789");
    info!("Listening for WebSocket connections on port 56789");

    let incoming_client_stream = stream::unfold(listener, |listener| async {
        let (stream, addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                error!("Failed to accept incoming TCP connection: {}", e);
                return None;
            }
        };

        info!("Accepted connection from {}", addr);

        Some((stream, listener))
    })
    .map(|stream: TcpStream| stream.compat())
    .boxed();

    incoming_client_stream
}

pub async fn create_websocket_and_bg_task() -> (
    WsDiscovery<Compat<TcpStream>>,
    PinnedLocalFuture<'static, Result<(), String>>,
) {
    let ws_discovery = WsDiscovery::new();
    let incoming_client_stream = create_tcp_client_stream().await;
    let ws_listen = ws_discovery.listen(incoming_client_stream).boxed_local();
    (ws_discovery, ws_listen)
}
