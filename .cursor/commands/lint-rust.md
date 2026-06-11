Run the narrow Rust quality gate.

Run `make lint-rust` (clippy for both `opnble` and `opnble-agent`, plus `cargo fmt --check`). Quote the actual pass/fail line. If it fails, group findings by crate and file.

Use this when only Rust files in `src/tauri/**/*.rs` or `src/agent/**/*.rs` changed. For combined frontend+Rust changes, use `/check` instead.
