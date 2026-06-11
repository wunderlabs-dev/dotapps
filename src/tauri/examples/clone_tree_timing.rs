//! Example binary to time `clone_tree_or_copy` against a real directory tree.
//!
//! Usage: `cargo run --example clone_tree_timing -- <src> <dst>`

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::indexing_slicing,
    clippy::unnecessary_wraps,
    reason = "manual timing example: prints results, validates argv shape, signature kept Result for ? ergonomics"
)]

use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} <src> <dst>", args[0]);
        std::process::exit(1);
    }
    let src = Path::new(&args[1]);
    let dst = Path::new(&args[2]);
    let start = Instant::now();
    opnble_lib::vm::clone::clone_tree_or_copy(src, dst).expect("clone");
    let elapsed = start.elapsed();
    println!("cloned in {elapsed:?}");
    Ok(())
}
