//! Run a command on the VM host via the agent (no container needed).
//!
//! Usage:
//!   cargo run -p opnble --example `vm_exec` -- "cat /etc/resolv.conf"
//!   cargo run -p opnble --example `vm_exec`                          # network diagnostics
//!
//! Requires the app to be running (make dev or make dev-all).
//! Works even when Podman is dead, since the command runs directly on the VM.

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("cannot create tokio runtime");
    rt.block_on(run());
}

#[expect(
    clippy::print_stdout,
    reason = "CLI diagnostic tool outputs to terminal"
)]
async fn run() {
    let home = std::env::var("HOME").expect("HOME not set");
    let socket = std::path::PathBuf::from(format!("{home}/.opnble/vm/agent.sock"));

    if !socket.exists() {
        print!("Agent socket not found. Is the app running? (make dev)");
        return;
    }

    let agent = opnble_lib::vm::agent_client::AgentClient::new(socket);

    match agent.ping().await {
        Ok(v) => println!("Agent v{v}"),
        Err(e) => {
            print!("Agent not reachable: {e}");
            return;
        }
    }

    let cmd = std::env::args().nth(1).unwrap_or_else(default_diag_cmd);

    match agent.exec_host(&cmd).await {
        Ok((exit_code, stdout, stderr)) => {
            if !stdout.is_empty() {
                print!("{stdout}");
            }
            if !stderr.is_empty() {
                print!("[stderr] {stderr}");
            }
            println!("(exit_code={exit_code})");
        }
        Err(e) => {
            print!("exec_host failed: {e}");
        }
    }
}

fn default_diag_cmd() -> String {
    [
        "echo '=== resolv.conf ==='",
        "cat /etc/resolv.conf 2>&1 || echo 'MISSING'",
        "echo '=== ip addr ==='",
        "ip addr 2>/dev/null || ifconfig 2>/dev/null || echo 'no ip tools'",
        "echo '=== ping gateway (192.168.127.1) ==='",
        "ping -c1 -W3 192.168.127.1 2>&1 || echo 'UNREACHABLE'",
        "echo '=== ping 8.8.8.8 ==='",
        "ping -c1 -W3 8.8.8.8 2>&1 || echo 'UNREACHABLE'",
        "echo '=== nslookup registry.npmjs.org ==='",
        "nslookup registry.npmjs.org 2>&1 || echo 'DNS FAILED'",
    ]
    .join(" && ")
}
