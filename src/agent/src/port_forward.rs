//! vsock port forwarding
//!
//! Multiplexes the vsock listener between ttrpc (agent RPC) and TCP port
//! forwarding. The first byte of each connection determines the protocol:
//! - `0x01`: port forwarding (next 2 bytes = target port, then bidirectional proxy)
//! - anything else: ttrpc (connection bridged to the internal Unix socket)

use std::io::{self, Read, Write};
use std::net::{self, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::thread;

use log::{error, info, warn};
use vsock::{VsockAddr, VsockListener, VsockStream};

use crate::error::AgentError;

/// Magic byte identifying a port forwarding connection.
///
/// Safe because valid ttrpc messages always start with 0x00: the high byte
/// of a big-endian u32 length field (max payload = 4MB = 0x00400000).
///
/// INVARIANT: this depends on `ttrpc::proto::MESSAGE_LENGTH_MAX` staying
/// at or below 16MB (0x00FFFFFF). If a future ttrpc version raises it
/// above that threshold, the first byte could be 0x01 for large messages,
/// breaking protocol detection. The constant is not re-exported, so we
/// cannot add a compile-time assertion. Verify on ttrpc upgrades.
const PORT_FORWARD_MAGIC: u8 = 0x01;

/// Internal Unix socket path where the ttrpc server listens.
pub const TTRPC_INTERNAL_SOCKET: &str = "/run/opnble/ttrpc.sock";

/// Bind a vsock listener during bootstrap.
///
/// Returns a structured error rather than panicking, so the caller can log
/// the failure and exit cleanly instead of producing a bare `.expect` panic
/// for what is an operational failure (already bound port, missing
/// `/dev/vsock`, missing `CAP_NET_BIND_SERVICE`, etc.).
pub fn bind_vsock(cid: u32, port: u32) -> Result<VsockListener, AgentError> {
    let addr = VsockAddr::new(cid, port);
    VsockListener::bind(&addr).map_err(|source| AgentError::VsockBind { cid, port, source })
}

/// Accept connections on a pre-bound vsock listener, routing each to either
/// the port forwarding handler or the ttrpc server (via internal Unix socket).
///
/// Blocks forever (until the process exits).
pub fn run_vsock_mux(listener: &VsockListener) {
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_connection(stream);
                });
            }
            Err(e) => {
                warn!("cannot accept vsock connection: {e}");
            }
        }
    }
}

/// Create the runtime directory and remove the stale ttrpc socket.
pub fn prepare_ttrpc_socket() {
    if let Some(parent) = std::path::Path::new(TTRPC_INTERNAL_SOCKET).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(TTRPC_INTERNAL_SOCKET);
}

/// Read the first byte and route the connection.
fn handle_connection(mut stream: VsockStream) {
    let mut buf = [0u8; 1];
    if let Err(e) = stream.read_exact(&mut buf) {
        error!("cannot read first byte: {e}");
        return;
    }

    let [first_byte] = buf;
    if first_byte == PORT_FORWARD_MAGIC {
        handle_port_forward(stream);
    } else {
        bridge_to_ttrpc(stream, first_byte);
    }
}

/// Handle a port forwarding connection.
///
/// Protocol:
///   client -> agent: `[0x01] [port_hi] [port_lo]`
///   agent -> client: [0x00] (success) or [0x01] (failure, then close)
///   then: bidirectional TCP proxy
fn handle_port_forward(mut vsock: VsockStream) {
    let mut port_bytes = [0u8; 2];
    if let Err(e) = vsock.read_exact(&mut port_bytes) {
        error!("cannot read target port: {e}");
        return;
    }

    let port = u16::from_be_bytes(port_bytes);
    info!("port forward request -> 127.0.0.1:{port}");

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match TcpStream::connect(addr) {
        Ok(target) => {
            if let Err(e) = vsock.write_all(&[0x00]) {
                error!("cannot send success byte: {e}");
                return;
            }
            let vsock_clone = match vsock.try_clone() {
                Ok(c) => c,
                Err(e) => {
                    error!("cannot clone vsock stream: {e}");
                    return;
                }
            };
            let tcp_clone = match target.try_clone() {
                Ok(c) => c,
                Err(e) => {
                    error!("cannot clone tcp stream: {e}");
                    return;
                }
            };
            bidirectional_copy(vsock, target, tcp_clone, vsock_clone);
        }
        Err(e) => {
            warn!("cannot connect to 127.0.0.1:{port}: {e}");
            let _ = vsock.write_all(&[0x01]);
        }
    }
}

/// Bridge a connection to the internal ttrpc Unix socket.
///
/// Writes the already-consumed first byte before proxying the rest.
fn bridge_to_ttrpc(vsock: VsockStream, first_byte: u8) {
    let mut ttrpc_conn = match UnixStream::connect(TTRPC_INTERNAL_SOCKET) {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot connect to ttrpc socket: {e}");
            return;
        }
    };

    if let Err(e) = ttrpc_conn.write_all(&[first_byte]) {
        error!("cannot write first byte to ttrpc: {e}");
        return;
    }

    let vsock_clone = match vsock.try_clone() {
        Ok(c) => c,
        Err(e) => {
            error!("cannot clone vsock stream: {e}");
            return;
        }
    };
    let unix_clone = match ttrpc_conn.try_clone() {
        Ok(c) => c,
        Err(e) => {
            error!("cannot clone unix stream: {e}");
            return;
        }
    };
    bidirectional_copy(vsock, ttrpc_conn, unix_clone, vsock_clone);
}

/// Copy data bidirectionally between two stream pairs.
///
/// Takes pre-cloned halves: `a_read`/`b_write` for one direction,
/// `b_read`/`a_write` for the other. Spawns two threads and joins both.
fn bidirectional_copy(
    mut a_read: impl Read + Send + 'static,
    mut b_write: impl Write + ShutdownWrite + Send + 'static,
    mut b_read: impl Read + Send + 'static,
    mut a_write: impl Write + ShutdownWrite + Send + 'static,
) {
    let t1 = thread::spawn(move || {
        let _ = io::copy(&mut a_read, &mut b_write);
        b_write.shutdown_write();
    });

    let t2 = thread::spawn(move || {
        let _ = io::copy(&mut b_read, &mut a_write);
        a_write.shutdown_write();
    });

    let _ = t1.join();
    let _ = t2.join();
}

/// Unifies the shutdown method across stream types for the generic proxy.
trait ShutdownWrite {
    fn shutdown_write(&self);
}

impl ShutdownWrite for TcpStream {
    fn shutdown_write(&self) {
        let _ = self.shutdown(net::Shutdown::Write);
    }
}

impl ShutdownWrite for UnixStream {
    fn shutdown_write(&self) {
        let _ = self.shutdown(net::Shutdown::Write);
    }
}

impl ShutdownWrite for VsockStream {
    fn shutdown_write(&self) {
        let _ = self.shutdown(net::Shutdown::Write);
    }
}
