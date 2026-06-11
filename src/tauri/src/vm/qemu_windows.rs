//! QEMU wrapper for Windows when WSL2 is not available
//!
//! Uses QEMU with WHPX (Windows Hypervisor Platform) acceleration for better performance.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use super::{VirtualMachine, VmError};
use crate::ssh;

pub struct QemuWindows {
    image_path: PathBuf,
    ssh_port: u16,
    memory_mb: u32,
    cpus: u32,
    process: Option<Child>,
}

impl QemuWindows {
    pub fn new(image_path: PathBuf, ssh_port: u16) -> Self {
        Self {
            image_path,
            ssh_port,
            memory_mb: 2048,
            cpus: 2,
            process: None,
        }
    }
}

impl VirtualMachine for QemuWindows {
    fn start(&mut self) -> Result<(), VmError> {
        if self.is_running() {
            return Ok(());
        }

        // Use Windows Hypervisor Platform (WHPX) for acceleration
        let child = Command::new("qemu-system-x86_64")
            .args([
                "-machine",
                "q35,accel=whpx",
                "-cpu",
                "max",
                "-smp",
                &self.cpus.to_string(),
                "-m",
                &format!("{}M", self.memory_mb),
                "-drive",
                &format!("file={},format=qcow2,if=virtio", self.image_path.display()),
                "-netdev",
                &format!("user,id=net0,hostfwd=tcp:127.0.0.1:{}-:22", self.ssh_port),
                "-device",
                "virtio-net-pci,netdev=net0",
                "-nographic",
                "-serial",
                "none",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| VmError::LaunchFailed {
                reason: format!("cannot spawn qemu: {e}"),
            })?;

        self.process = Some(child);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), VmError> {
        if let Some(mut process) = self.process.take() {
            // Try graceful shutdown via SSH first
            if let Ok(session) = ssh::connect(self.ssh_port) {
                let _ = ssh::run_command(&session, "sudo poweroff");
            }

            // Wait a bit then force kill if needed
            thread::sleep(Duration::from_secs(5));
            let _ = process.kill();
            let _ = process.wait();
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        if let Some(ref process) = self.process {
            // On Windows, check if process is still running using tasklist
            match Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", process.id())])
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    stdout.contains(&process.id().to_string())
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }

    fn ssh_port(&self) -> u16 {
        self.ssh_port
    }

    fn ssh_address(&self) -> (String, u16) {
        ("127.0.0.1".to_string(), self.ssh_port)
    }

    fn wait_for_ready(&self) -> Result<(), VmError> {
        // Wait for SSH to be available (longer timeout for QEMU)
        for _ in 0..120 {
            if ssh::is_ready(self.ssh_port) {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err(VmError::ConnectionFailed {
            reason: "cannot reach vm after 120s".to_string(),
        })
    }
}
