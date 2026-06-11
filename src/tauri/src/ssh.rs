use std::io::Read;
use std::net::TcpStream;
use std::time::Duration;

use ssh2::Session;

use crate::vm::VmError;

const SSH_USER: &str = "opnble";
const SSH_PASSWORD: &str = "opnble";

fn connection_err(context: &str) -> impl FnOnce(ssh2::Error) -> VmError + '_ {
    move |e| VmError::ConnectionFailed {
        reason: format!("cannot {context}: {e}"),
    }
}

fn command_err(context: &str) -> impl FnOnce(ssh2::Error) -> VmError + '_ {
    move |e| VmError::CommandFailed {
        reason: format!("cannot {context}: {e}"),
    }
}

fn command_io_err(context: &str) -> impl FnOnce(std::io::Error) -> VmError + '_ {
    move |e| VmError::CommandFailed {
        reason: format!("cannot {context}: {e}"),
    }
}

/// Create an SSH session to the VM at the specified host and port.
///
/// This is the primary connection method that supports both:
/// - QEMU: connect_to("127.0.0.1", 2222) for port-forwarded SSH
/// - vfkit: connect_to("192.168.64.x", 22) for direct VM IP access
pub fn connect_to(host: &str, port: u16) -> Result<Session, VmError> {
    let addr = format!("{host}:{port}");
    let tcp = TcpStream::connect(&addr).map_err(|e| VmError::ConnectionFailed {
        reason: format!("cannot connect to {addr}: {e}"),
    })?;

    tcp.set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| VmError::ConnectionFailed {
            reason: format!("cannot set read timeout: {e}"),
        })?;
    tcp.set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| VmError::ConnectionFailed {
            reason: format!("cannot set write timeout: {e}"),
        })?;

    let mut session = Session::new().map_err(connection_err("create ssh session"))?;

    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(connection_err("complete ssh handshake"))?;

    session
        .userauth_password(SSH_USER, SSH_PASSWORD)
        .map_err(connection_err("authenticate ssh session"))?;

    if !session.authenticated() {
        return Err(VmError::ConnectionFailed {
            reason: "cannot authenticate ssh session".to_string(),
        });
    }

    Ok(session)
}

/// Create an SSH session to the VM on localhost at the specified port.
///
/// Convenience wrapper for QEMU-style port forwarding where SSH is
/// forwarded to localhost:port.
pub fn connect(port: u16) -> Result<Session, VmError> {
    connect_to("127.0.0.1", port)
}

/// Check if SSH is ready at the specified host and port.
pub fn is_ready_at(host: &str, port: u16) -> bool {
    match connect_to(host, port) {
        Ok(session) => {
            // Try to run a simple command
            match run_command(&session, "echo ready") {
                Ok(output) => output.trim() == "ready",
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

/// Check if SSH is ready on localhost at the specified port.
///
/// Convenience wrapper for QEMU-style port forwarding.
pub fn is_ready(port: u16) -> bool {
    is_ready_at("127.0.0.1", port)
}

/// Run a command on the VM via SSH
pub fn run_command(session: &Session, command: &str) -> Result<String, VmError> {
    let mut channel = session
        .channel_session()
        .map_err(command_err("open channel"))?;

    channel
        .exec(command)
        .map_err(command_err("execute command"))?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .map_err(command_io_err("read output"))?;

    channel.wait_close().map_err(command_err("close channel"))?;

    let exit_status = channel
        .exit_status()
        .map_err(command_err("get exit status"))?;

    if exit_status != 0 {
        let mut stderr = String::new();
        let _ = channel.stderr().read_to_string(&mut stderr);
        return Err(VmError::CommandFailed {
            reason: format!("command exited with status {exit_status}: {stderr}"),
        });
    }

    Ok(output)
}
