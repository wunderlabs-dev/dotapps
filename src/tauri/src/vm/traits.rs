//! VM abstraction trait and platform detection

use super::VmError;

/// Trait for VM implementations
pub trait VirtualMachine: Send + Sync {
    /// Start the virtual machine
    fn start(&mut self) -> Result<(), VmError>;

    /// Stop the virtual machine
    fn stop(&mut self) -> Result<(), VmError>;

    /// Check if the VM is currently running
    fn is_running(&self) -> bool;

    /// Get the SSH port for connecting to the VM
    fn ssh_port(&self) -> u16;

    /// Get the SSH address (host, port) for connecting to the VM.
    ///
    /// Different VM types have different networking models:
    /// - QEMU: Uses port forwarding, returns ("127.0.0.1", `forwarded_port`)
    /// - `VfkitVM`: Uses NAT with DHCP, returns (`vm_ip`, 22)
    fn ssh_address(&self) -> (String, u16);

    /// Wait for the VM to be ready.
    ///
    /// Note: `VmGateway` handles the actual readiness wait via its own vsock
    /// polling loop, so implementations may be no-ops.
    fn wait_for_ready(&self) -> Result<(), VmError>;

    /// Get the Unix socket path for vsock communication (if supported).
    /// Returns None if this VM type uses SSH instead.
    fn vsock_socket(&self) -> Option<&std::path::Path> {
        None // Default: not supported
    }
}
