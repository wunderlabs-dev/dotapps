// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(e) = opnble_lib::run() {
        tracing::error!(error = %e, "bootstrap failed");
        std::process::exit(1);
    }
}
