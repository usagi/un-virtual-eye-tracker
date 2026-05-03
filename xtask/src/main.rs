use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod check_clean_room;
mod make_release_package;

/// Repository helper tasks (replacement for the previous `tools/*.ps1` scripts).
#[derive(Debug, Parser)]
#[command(name = "xtask", about = "UNVET repository task runner", version)]
struct Cli {
 #[command(subcommand)]
 command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
 /// Scan tracked source directories for clean-room policy violations.
 CheckCleanRoom {
  /// Repository root (defaults to the workspace root).
  #[arg(long, value_name = "PATH")]
  repo_root: Option<PathBuf>,
 },
 /// Build the portable release package (Windows zip).
 MakeReleasePackage {
  /// Repository root (defaults to the workspace root).
  #[arg(long, value_name = "PATH")]
  repo_root: Option<PathBuf>,
  /// Override the version embedded in the package name.
  #[arg(long, value_name = "VERSION")]
  version: Option<String>,
  /// Directory in which the resulting zip is written.
  #[arg(long, value_name = "PATH")]
  output_dir: Option<PathBuf>,
  /// Skip `cargo build --release` and reuse existing artifacts.
  #[arg(long)]
  skip_build: bool,
  /// Keep the staging directory after the zip is produced.
  #[arg(long)]
  keep_staging: bool,
 },
}

fn workspace_root() -> PathBuf {
 // CARGO_MANIFEST_DIR points at the xtask crate; its parent is the workspace root.
 PathBuf::from(env!("CARGO_MANIFEST_DIR"))
  .parent()
  .map(PathBuf::from)
  .expect("xtask crate must live inside the workspace")
}

fn main() -> Result<()> {
 let cli = Cli::parse();
 match cli.command {
  Command::CheckCleanRoom { repo_root } => {
   let root = repo_root.unwrap_or_else(workspace_root);
   check_clean_room::run(&root)
  },
  Command::MakeReleasePackage {
   repo_root,
   version,
   output_dir,
   skip_build,
   keep_staging,
  } => {
   let root = repo_root.unwrap_or_else(workspace_root);
   make_release_package::run(make_release_package::Options {
    repo_root: root,
    version,
    output_dir,
    skip_build,
    keep_staging,
   })
  },
 }
}
