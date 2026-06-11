//! Static configuration for the local MCP listener.
//!
//! The MCP listener always binds on the loopback interface so the editor /
//! agent process can reach it without exposing it to the network. The port is
//! hard-coded so the `~/.opnble/mcp.json` config that the editor reads stays
//! stable across restarts and across machines (the user's installed Cursor /
//! Claude config keeps working).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// Loopback port for the MCP listener.
///
/// Picked from the IANA "dynamic / private" range (49152-65535) and away from
/// other common dev-tool ports. If a future Opnble release ever changes this,
/// the editor config writer also has to migrate `~/.opnble/mcp.json`.
pub const PORT: u16 = 47821;

/// HTTP path the streamable MCP transport is mounted under.
pub const MCP_PATH: &str = "/mcp";

/// `127.0.0.1:PORT` as a `SocketAddr`.
#[inline]
pub fn bind_address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT)
}

/// The full URL clients connect to, e.g. `http://127.0.0.1:47821/mcp`.
#[inline]
pub fn mcp_url() -> String {
    format!("http://127.0.0.1:{PORT}{MCP_PATH}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_address_is_loopback() {
        let addr = bind_address();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), PORT);
    }

    #[test]
    fn mcp_url_matches_constants() {
        assert_eq!(mcp_url(), format!("http://127.0.0.1:{PORT}{MCP_PATH}"));
    }
}
