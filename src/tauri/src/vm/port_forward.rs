//! Port forwarding via vsock
//!
//! Proxies TCP connections from `localhost:{port}` on the host to containers
//! inside the VM. Each TCP connection opens a new vsock connection to the
//! agent, which connects to the container's port.

use std::path::{Path, PathBuf};

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UnixStream};
use tokio::task::JoinHandle;

use super::VmError;

/// Magic byte identifying a port forwarding connection (matches agent).
const PORT_FORWARD_MAGIC: u8 = 0x01;

fn conn_err(context: &str) -> impl FnOnce(std::io::Error) -> VmError + '_ {
    move |e| VmError::ConnectionFailed {
        reason: format!("cannot {context}: {e}"),
    }
}

/// Start forwarding `host_port` on localhost to `container_port` inside the VM.
///
/// Returns a handle that can be aborted to stop forwarding.
pub fn start_forwarding(vsock_path: &Path, host_port: u16, container_port: u16) -> JoinHandle<()> {
    let vsock_path = vsock_path.to_path_buf();

    tokio::spawn(async move {
        if let Err(e) = run_listener(vsock_path, host_port, container_port).await {
            tracing::error!("port forward listener for port {host_port} failed: {e}");
        }
    })
}

/// Stop forwarding by aborting the listener task.
pub fn stop_forwarding(handle: &JoinHandle<()>) {
    handle.abort();
}

/// Run a TCP listener that proxies each connection through vsock.
async fn run_listener(
    vsock_path: PathBuf,
    host_port: u16,
    container_port: u16,
) -> Result<(), VmError> {
    let listener = TcpListener::bind(format!("127.0.0.1:{host_port}"))
        .await
        .map_err(conn_err(&format!("bind to 127.0.0.1:{host_port}")))?;

    tracing::info!("port forward: 127.0.0.1:{host_port} -> VM:{container_port}");

    loop {
        let (tcp_stream, peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("accept error on port {host_port}: {e}");
                continue;
            }
        };

        tracing::debug!("port forward connection from {peer} for port {container_port}");

        let vsock = vsock_path.clone();
        tokio::spawn(async move {
            if let Err(e) = proxy_connection(vsock, tcp_stream, container_port).await {
                tracing::debug!("port forward proxy ended: {e}");
            }
        });
    }
}

/// Proxy a single TCP connection through vsock to the VM.
///
/// Opens a new vsock connection, performs the port forward handshake,
/// then copies data bidirectionally.
async fn proxy_connection(
    vsock_path: PathBuf,
    mut tcp: tokio::net::TcpStream,
    container_port: u16,
) -> Result<(), VmError> {
    // Connect to the agent's vsock socket
    let mut vsock = UnixStream::connect(&vsock_path)
        .await
        .map_err(conn_err(&format!(
            "connect to vsock at {}",
            vsock_path.display()
        )))?;

    // Send the port forward handshake: [magic] [port_hi] [port_lo]
    let [port_hi, port_lo] = container_port.to_be_bytes();
    let handshake = [PORT_FORWARD_MAGIC, port_hi, port_lo];

    vsock
        .write_all(&handshake)
        .await
        .map_err(conn_err("send handshake"))?;

    // Read the response byte
    let mut buf = [0u8; 1];
    vsock
        .read_exact(&mut buf)
        .await
        .map_err(conn_err("read handshake response"))?;

    let [status] = buf;
    if status != 0x00 {
        return Err(VmError::ConnectionFailed {
            reason: format!(
                "agent refused port forward to {container_port} (response: {status:#04x})"
            ),
        });
    }

    // Bidirectional copy until either side closes
    io::copy_bidirectional(&mut tcp, &mut vsock)
        .await
        .map_err(conn_err("proxy connection"))?;

    Ok(())
}
