mod archive;
mod manifest;
mod pack;
mod publish;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vibox", version, about = "Pack and publish vibox apps")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build the project's Docker image and pack it into a .vibox archive
    Pack {
        /// Project directory containing vibox.json and a Dockerfile
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Output path (defaults to {slug}-{version}.vibox in the current directory)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Upload a .vibox archive to the registry
    Publish {
        /// Archive to publish (defaults to the newest *.vibox in the current directory)
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, help = "Registry base URL (defaults to $VIBOX_REGISTRY)")]
        registry: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Pack { dir, out } => pack::run(&dir, out.as_deref()),
        Command::Publish { file, registry } => publish::run(file.as_deref(), registry.as_deref()),
    }
}

/// User-facing CLI output. Centralized so the workspace `print_stdout` deny
/// is suppressed in exactly one place.
#[expect(
    clippy::print_stdout,
    reason = "user-facing CLI output belongs on stdout"
)]
pub(crate) fn say(message: &str) {
    println!("{message}");
}
