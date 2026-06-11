//! QEMU wrapper for Intel Macs
//!
//! Uses QEMU with HVF (Hypervisor.framework) acceleration for better performance.

use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use super::{VirtualMachine, VmError};

pub struct QemuVM {
    image_path: PathBuf,
    ssh_port: u16,
    memory_mb: u32,
    cpus: u32,
    process: Option<Child>,
}

impl QemuVM {
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

impl VirtualMachine for QemuVM {
    fn start(&mut self) -> Result<(), VmError> {
        if self.is_running() {
            return Ok(());
        }

        let qemu_binary = if cfg!(target_arch = "x86_64") {
            "qemu-system-x86_64"
        } else {
            "qemu-system-aarch64"
        };

        let child = Command::new(qemu_binary)
            .args([
                "-machine",
                "q35,accel=hvf",
                "-cpu",
                "host",
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
            // Force kill the QEMU process
            // (Graceful shutdown via SSH removed, agent handles shutdown now)
            let _ = process.kill();
            let _ = process.wait();
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        if let Some(ref process) = self.process {
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
        self.ssh_port
    }

    fn ssh_address(&self) -> (String, u16) {
        // QEMU uses port forwarding: localhost:ssh_port -> VM:22
        ("127.0.0.1".to_string(), self.ssh_port)
    }

    fn wait_for_ready(&self) -> Result<(), VmError> {
        // Wait for SSH port to be reachable via TCP (longer timeout for QEMU)
        let addr = format!("127.0.0.1:{}", self.ssh_port);
        let socket_addr = addr
            .parse()
            .expect("static 127.0.0.1:<port> is always valid");
        for _ in 0..120 {
            if TcpStream::connect_timeout(&socket_addr, Duration::from_secs(2)).is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err(VmError::ConnectionFailed {
            reason: "cannot reach vm after 120s".to_string(),
        })
    }
}
