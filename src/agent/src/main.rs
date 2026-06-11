use std::sync::Arc;
use std::time::Duration;

use log::info;
use nix::sys::signal::{self, SigSet, Signal};

use opnble_agent::generated::agent_ttrpc::create_agent;
use opnble_agent::port_forward;
use opnble_agent::service::ContainerAgent;

/// `VMADDR_CID_ANY` -- listen on any CID (guest-side wildcard)
const VMADDR_CID_ANY: u32 = 0xFFFF_FFFF;
/// Port the agent listens on (must match the host-side connect port)
const VSOCK_PORT: u32 = 1024;

/// Start `podman system service` if the socket doesn't exist yet.
///
/// The bollard client needs the podman REST API socket. On Alpine VMs
/// there is no systemd socket activation, so we start it manually.
/// `--time=0` means no idle timeout (runs until killed).
///
/// Returns an error if the socket does not appear within the timeout,
/// so the agent fails fast rather than producing cryptic bollard errors.
fn ensure_podman_socket(socket_path: &str) -> Result<(), String> {
    // Check if socket is alive (not just if the file exists).
    // A stale socket file from a crashed Podman must be cleaned up.
    if is_socket_alive(socket_path) {
        info!("podman socket alive at {socket_path}");
        return Ok(());
    }

    // Remove stale socket file and kill any zombie Podman process
    if std::path::Path::new(socket_path).exists() {
        info!("podman socket file exists but is dead, cleaning up");
        let _ = std::fs::remove_file(socket_path);
        let _ = std::process::Command::new("pkill")
            .args(["-9", "podman"])
            .output();
        std::thread::sleep(Duration::from_millis(500));
    }

    info!("starting podman system service at {socket_path}");

    if let Some(parent) = std::path::Path::new(socket_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let uri = format!("unix://{socket_path}");
    let mut child = std::process::Command::new("podman")
        .args(["system", "service", "--time=0", &uri])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start podman system service: {e}"))?;

    info!("podman system service spawned (pid {})", child.id());

    // Wait for socket to become connectable (not just file existence)
    for _ in 0..300 {
        if is_socket_alive(socket_path) {
            info!("podman socket ready");
            // Detach: the child stays running for the rest of the agent's life.
            // We deliberately do not call wait() here.
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Timeout: reap the spawned child so we do not leak a long-running
    // `podman system service` process whose socket never came up.
    let pid = child.id();
    if let Err(e) = child.kill() {
        log::warn!("failed to kill stalled podman child (pid {pid}): {e}");
    }
    if let Err(e) = child.wait() {
        log::warn!("failed to reap stalled podman child (pid {pid}): {e}");
    }

    Err(format!(
        "podman socket did not become connectable at {socket_path} within 30s"
    ))
}

/// Check if a Unix socket is alive by attempting to connect.
fn is_socket_alive(path: &str) -> bool {
    use std::os::unix::net::UnixStream;
    std::path::Path::new(path).exists() && UnixStream::connect(path).is_ok()
}

fn main() {
    env_logger::init();
    if let Err(e) = run() {
        log::error!("bootstrap failed: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Block SIGTERM and SIGINT BEFORE starting the server so that ttrpc
    // worker threads inherit the blocked mask and only our dedicated
    // signal thread receives these signals.
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGTERM);
    mask.add(Signal::SIGINT);
    signal::pthread_sigmask(signal::SigmaskHow::SIG_BLOCK, Some(&mask), None)?;

    // Create tokio runtime for bollard async calls.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let handle = runtime.handle().clone();

    // Prepare internal Unix socket for ttrpc (remove stale socket from previous run)
    port_forward::prepare_ttrpc_socket();

    // Start ttrpc server on an internal Unix socket.
    // The vsock mux (below) bridges vsock connections to this socket,
    // routing port forwarding connections to the port_forward handler.
    let agent = Arc::new(ContainerAgent::new(handle));
    let agent_service = create_agent(agent);

    let socket = port_forward::TTRPC_INTERNAL_SOCKET;
    let ttrpc_addr = format!("unix://{socket}");
    info!("starting ttrpc server on {ttrpc_addr}");

    let mut server = ttrpc::Server::new()
        .bind(&ttrpc_addr)?
        .register_service(agent_service);

    server.start()?;
    info!("ttrpc server running on internal socket");

    // Bind the vsock listener on the main thread. A bind failure is an
    // operational error (port busy, missing /dev/vsock, missing capability)
    // surfaced via AgentError; ? propagates it to run() which logs and
    // exits non-zero, consistent with the other ?-propagated startup above.
    let vsock_listener = port_forward::bind_vsock(VMADDR_CID_ANY, VSOCK_PORT)?;
    info!("vsock mux bound on port {VSOCK_PORT}");

    // Accept loop runs in a background thread, routing connections to either
    // the ttrpc server or the port forwarding handler based on the first byte.
    std::thread::spawn(move || {
        port_forward::run_vsock_mux(&vsock_listener);
    });

    // Now ensure podman is ready (retries for cold boot)
    let socket_path = "/run/podman/podman.sock";
    for attempt in 0..5 {
        match ensure_podman_socket(socket_path) {
            Ok(()) => break,
            Err(e) => {
                info!("podman socket attempt {attempt} failed: {e}");
                if attempt < 4 {
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }
    }
    info!("opnble-agent fully ready");

    // Spawn the sigwait thread and block until SIGTERM or SIGINT
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let _ = mask.wait();
        let _ = tx.send(());
    });
    let _ = rx.recv();

    info!("shutting down opnble-agent");
    server.shutdown();
    Ok(())
}
