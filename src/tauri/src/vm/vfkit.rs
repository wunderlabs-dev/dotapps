//! vfkit wrapper for Apple Silicon Macs
//!
//! Uses Apple's Virtualization.framework via vfkit CLI for fast VM boot (~2s).
//! Communication with the guest is via vsock (Unix socket), not SSH.
#![expect(
    clippy::disallowed_types,
    reason = "std::process::Child is !Send, requires sync Mutex"
)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use super::{VirtualMachine, VmError};

/// Resolve the path of a Tauri externalBin sidecar.
///
/// Defers to `tauri-plugin-shell` so the target-triple suffix lookup matches
/// what the bundler produces (`Contents/MacOS/<name>` in production, plus the
/// `target/debug/<name>-<triple>` convention in dev). Spawning still happens
/// through `std::process::Command` so the existing `Child`-based lifecycle
/// stays intact.
fn sidecar_path(app: &AppHandle, name: &str) -> Result<PathBuf, VmError> {
    let cmd: Command = app
        .shell()
        .sidecar(name)
        .map_err(|e| VmError::ConfigInvalid {
            reason: format!("cannot resolve {name} sidecar: {e}"),
        })?
        .into();
    let path = PathBuf::from(cmd.get_program());
    if !path.exists() {
        return Err(VmError::ConfigInvalid {
            reason: format!("sidecar {name} not found at {}", path.display()),
        });
    }
    Ok(path)
}

/// MAC address matching gvproxy's default DHCP lease for 192.168.127.2
const VM_MAC_ADDRESS: &str = "5a:94:ef:e4:0c:ee";

/// Find an available TCP port by binding to port 0 and letting the OS assign
/// one.
///
/// Surfaces the OS error as `VmError::ConfigInvalid` instead of panicking,
/// since binding can legitimately fail on a host with no IPv4 loopback or
/// under tight sandbox limits.
fn find_available_port() -> Result<u16, VmError> {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| VmError::ConfigInvalid {
            reason: format!("cannot bind ephemeral port: {e}"),
        })?;
    let addr = listener.local_addr().map_err(|e| VmError::ConfigInvalid {
        reason: format!("cannot read ephemeral port: {e}"),
    })?;
    Ok(addr.port())
}

/// Kill the process recorded in `pidfile` if it is still alive and still the
/// expected binary, then remove the pidfile.
///
/// Reaps a `vfkit`/`gvproxy` child orphaned by a non-graceful host exit: when
/// `cargo tauri dev` kills the host on hot rebuild, `VmLifecycle::stop` never
/// runs, so the previous VM keeps holding the fixed MAC, EFI store, and sockets
/// and the next boot never reaches the agent. The comm check guards against a
/// recycled PID now owned by an unrelated process.
fn reap_stale_process(pidfile: &Path, expected_comm: &str) {
    let Ok(contents) = std::fs::read_to_string(pidfile) else {
        return;
    };
    let Ok(pid) = contents.trim().parse::<u32>() else {
        let _ = std::fs::remove_file(pidfile);
        return;
    };
    if process_comm_matches(pid, expected_comm) {
        tracing::warn!("reaping orphaned {expected_comm} from previous session (pid {pid})");
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
    }
    let _ = std::fs::remove_file(pidfile);
}

fn process_comm_matches(pid: u32, expected_comm: &str) -> bool {
    let Ok(output) = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout).contains(expected_comm)
}

fn write_pidfile(pidfile: &Path, pid: u32) {
    if let Err(e) = std::fs::write(pidfile, pid.to_string()) {
        tracing::warn!("cannot write pidfile {}: {e}", pidfile.display());
    }
}

pub struct VfkitVM {
    vm_image_path: PathBuf,
    memory_mb: u32,
    cpus: u32,
    process: Mutex<Option<Child>>,
    vfkit_path: PathBuf,
    gvproxy_path: PathBuf,
    mac_address: String,
    /// REST API port for vfkit control
    rest_port: u16,
    /// Unix socket path for vsock bridge (vfkit creates this)
    vsock_socket_path: PathBuf,
    /// Host-side repos directory shared via `VirtioFS`
    repos_path: PathBuf,
    /// Unix socket path for gvproxy API (port forwarding control)
    gvproxy_api_path: PathBuf,
    /// Unix socket path for gvproxy-to-vfkit network bridge
    gvproxy_vfkit_path: PathBuf,
    /// gvproxy process handle
    gvproxy_process: Mutex<Option<Child>>,
}

impl VfkitVM {
    pub fn new(
        app: &AppHandle,
        vm_image_path: PathBuf,
        _ssh_port: u16,
        repos_path: PathBuf,
    ) -> Result<Self, VmError> {
        let vfkit_path = sidecar_path(app, "vfkit")?;
        let gvproxy_path = sidecar_path(app, "gvproxy")?;

        let vsock_socket_path = vm_image_path
            .parent()
            .unwrap_or(&vm_image_path)
            .join("agent.sock");

        let gvproxy_api_path =
            crate::constants::paths::gvproxy_api_socket().map_err(|e| VmError::ConfigInvalid {
                reason: format!("cannot resolve gvproxy API socket path: {e}"),
            })?;
        let gvproxy_vfkit_path = crate::constants::paths::gvproxy_vfkit_socket().map_err(|e| {
            VmError::ConfigInvalid {
                reason: format!("cannot resolve gvproxy vfkit socket path: {e}"),
            }
        })?;

        Ok(Self {
            vm_image_path,
            memory_mb: 2048,
            cpus: 2,
            process: Mutex::new(None),
            vfkit_path,
            gvproxy_path,
            mac_address: VM_MAC_ADDRESS.to_string(),
            rest_port: find_available_port()?,
            vsock_socket_path,
            repos_path,
            gvproxy_api_path,
            gvproxy_vfkit_path,
            gvproxy_process: Mutex::new(None),
        })
    }
}

impl VirtualMachine for VfkitVM {
    fn start(&mut self) -> Result<(), VmError> {
        if self.is_running() {
            return Ok(());
        }

        let vfkit_pidfile = self.vsock_socket_path.with_file_name("vfkit.pid");
        let gvproxy_pidfile = self.gvproxy_api_path.with_file_name("gvproxy.pid");

        // Reap a VM orphaned by a non-graceful host exit before reusing the
        // fixed MAC, EFI store, and sockets it would otherwise still hold.
        reap_stale_process(&vfkit_pidfile, "vfkit");
        reap_stale_process(&gvproxy_pidfile, "gvproxy");

        // Remove stale sockets from a previous crash
        let _ = std::fs::remove_file(&self.vsock_socket_path);
        let _ = std::fs::remove_file(&self.gvproxy_api_path);
        let _ = std::fs::remove_file(&self.gvproxy_vfkit_path);

        // Start gvproxy (must be running before vfkit connects)
        let gvproxy_log = self.gvproxy_api_path.with_file_name("gvproxy.log");
        let gvproxy_child = Command::new(&self.gvproxy_path)
            .args([
                "-debug",
                &format!("-log-file={}", gvproxy_log.display()),
                &format!(
                    "-pcap={}",
                    self.gvproxy_api_path
                        .with_file_name("gvproxy-live.pcap")
                        .display()
                ),
                &format!("-listen=unix://{}", self.gvproxy_api_path.display()),
                &format!(
                    "-listen-vfkit=unixgram://{}",
                    self.gvproxy_vfkit_path.display()
                ),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr({
                let log = self.gvproxy_api_path.with_file_name("gvproxy-stderr.log");
                std::fs::File::create(&log).map_or_else(|_| Stdio::piped(), Stdio::from)
            })
            .spawn()
            .map_err(|e| VmError::LaunchFailed {
                reason: format!("cannot start gvproxy: {e}"),
            })?;

        let gvproxy_pid = gvproxy_child.id();
        tracing::info!("gvproxy started (pid {gvproxy_pid})");
        write_pidfile(&gvproxy_pidfile, gvproxy_pid);
        *self.gvproxy_process.lock().expect("gvproxy mutex poisoned") = Some(gvproxy_child);

        // Give gvproxy time to create its sockets and be ready
        thread::sleep(Duration::from_secs(1));

        // Check if image exists
        if !self.vm_image_path.exists() {
            return Err(VmError::ImageNotFound {
                path: self.vm_image_path.clone(),
            });
        }

        // Create EFI variable store path
        let efi_store = self
            .vm_image_path
            .parent()
            .unwrap_or(&self.vm_image_path)
            .join("vfkit-efi-store");

        // Build vsock and VirtioFS device strings
        let vsock_device = format!(
            "virtio-vsock,port={},socketURL={},connect",
            crate::constants::vm::AGENT_VSOCK_PORT,
            self.vsock_socket_path.display()
        );
        let virtiofs_device = format!(
            "virtio-fs,sharedDir={},mountTag={}",
            self.repos_path.display(),
            crate::constants::vm::VIRTIOFS_MOUNT_TAG
        );

        // vfkit command with:
        // - EFI bootloader for booting disk images
        // - virtio-blk for the disk
        // - virtio-net via gvproxy for networking and port forwarding
        // - virtio-rng for random number generation
        // - virtio-vsock for host-guest communication via Unix socket
        // - virtio-fs for sharing repos directory with the guest
        // - REST API for VM control
        let child = Command::new(&self.vfkit_path)
            .args([
                "--cpus",
                &self.cpus.to_string(),
                "--memory",
                &self.memory_mb.to_string(),
                "--bootloader",
                &format!("efi,variable-store={},create", efi_store.display()),
                "--device",
                &format!("virtio-blk,path={}", self.vm_image_path.display()),
                "--device",
                &format!(
                    "virtio-net,unixSocketPath={},mac={}",
                    self.gvproxy_vfkit_path.display(),
                    self.mac_address
                ),
                "--device",
                "virtio-rng",
                "--device",
                &format!(
                    "virtio-serial,logFilePath={}",
                    self.vm_image_path.with_file_name("console.log").display()
                ),
                "--device",
                &vsock_device,
                "--device",
                &virtiofs_device,
                "--restful-uri",
                &format!("tcp://127.0.0.1:{}", self.rest_port),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| VmError::LaunchFailed {
                reason: format!("cannot spawn vfkit: {e}"),
            })?;

        write_pidfile(&vfkit_pidfile, child.id());
        *self.process.lock().expect("process mutex poisoned") = Some(child);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), VmError> {
        let mut process_guard = self.process.lock().expect("process mutex poisoned");
        if let Some(mut process) = process_guard.take() {
            // Try graceful shutdown via REST API first
            let stop_url = format!("http://127.0.0.1:{}/vm/state", self.rest_port);
            let _ = std::process::Command::new("curl")
                .args([
                    "-s",
                    "-X",
                    "POST",
                    "-H",
                    "Content-Type: application/json",
                    "-d",
                    r#"{"state":"Stop"}"#,
                    &stop_url,
                ])
                .output();

            // Wait for graceful shutdown
            thread::sleep(Duration::from_secs(3));

            // Force kill if still running
            let _ = process.kill();
            let _ = process.wait();
        }

        // Stop gvproxy
        let mut gvproxy_guard = self.gvproxy_process.lock().expect("gvproxy mutex poisoned");
        if let Some(mut gvproxy) = gvproxy_guard.take() {
            let _ = gvproxy.kill();
            let _ = gvproxy.wait();
        }

        let _ = std::fs::remove_file(self.vsock_socket_path.with_file_name("vfkit.pid"));
        let _ = std::fs::remove_file(self.gvproxy_api_path.with_file_name("gvproxy.pid"));

        Ok(())
    }

    fn is_running(&self) -> bool {
        let process_guard = self.process.lock().expect("process mutex poisoned");
        if let Some(ref process) = *process_guard {
            // Check if process is still running
            match Command::new("ps")
                .args(["-p", &process.id().to_string()])
                .output()
            {
                Ok(output) => output.status.success(),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    fn ssh_port(&self) -> u16 {
        // SSH is unused; vfkit communicates via vsock. Stub for trait compliance.
        22
    }

    fn ssh_address(&self) -> (String, u16) {
        // SSH is unused; vfkit communicates via vsock. Stub for trait compliance.
        ("127.0.0.1".to_string(), 22)
    }

    fn wait_for_ready(&self) -> Result<(), VmError> {
        // Readiness checking is handled by VmGateway::start() which polls
        // the vsock socket on a spawn_blocking thread to avoid holding the VM lock.
        Ok(())
    }

    fn vsock_socket(&self) -> Option<&std::path::Path> {
        Some(&self.vsock_socket_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_available_port() {
        let port = find_available_port().expect("loopback bind should succeed in tests");
        assert!(port > 0, "should find an available port");
    }
}
