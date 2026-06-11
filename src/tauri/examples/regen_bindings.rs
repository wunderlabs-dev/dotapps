//! Regenerate `src/web/gen/tauri.ts` without spinning up the Tauri runtime.
//!
//! In the normal dev workflow, `cargo tauri dev` triggers the
//! `#[cfg(debug_assertions)] specta_builder.export(...)` call in
//! `app::register::run_macos`, which writes the TS bindings as a side effect
//! of launching the app. That path also boots the VM, mounts the tray, and
//! holds the user's dev session hostage. This example builds the same specta
//! `Builder` and calls just `.export(...)`, then exits — no Tauri runtime,
//! no VM, no tray.
//!
//! Usage (from `src/tauri/`):
//!
//! ```sh
//! cargo run --example regen_bindings
//! ```
//!
//! macOS-only: the example mirrors the macOS command set because that is the
//! superset the bindings ship with. Windows/Linux entry points are stubs on
//! macOS so the wire shape is identical.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "manual regen example: prints progress so CI/devs can see what happened"
)]

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = opnble_lib::macos_specta_builder();
    let out_path = "../web/gen/tauri.ts";
    builder.export(opnble_lib::typescript_export_config(), out_path)?;
    println!("wrote bindings to {out_path}");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("regen_bindings only runs on macOS (matches the debug auto-export path).");
    std::process::exit(1);
}
