use anyhow::{Context, Result, anyhow, bail};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

const COPY_BUFFER_SIZE: usize = 64 * 1024;

pub struct Options {
 pub repo_root: PathBuf,
 pub version: Option<String>,
 pub output_dir: Option<PathBuf>,
 pub skip_build: bool,
 pub keep_staging: bool,
}

const REQUIRED_RELEASE_FILES: &[&str] = &[
 "unvet-desktop.exe",
 "unvet-cli.exe",
 "unvet_desktop_lib.dll",
 "NPClient64.dll",
 "NPClient.dll",
 "TrackIR.exe",
 "unvet-uninstall-compatible-layers.exe",
];

const OPTIONAL_ROOT_FILES: &[&str] = &["README.md", "LICENSE", "THIRD_PARTY_NOTICES.md"];

pub fn run(opts: Options) -> Result<()> {
 let repo_root = opts
  .repo_root
  .canonicalize()
  .with_context(|| format!("failed to resolve repository root: {}", opts.repo_root.display()))?;

 let version = match opts.version {
  Some(v) if !v.trim().is_empty() => v,
  _ => default_version(&repo_root)?,
 };

 let package_name = format!("unvet-{}", version);
 let release_dir = repo_root.join("target/release");
 let staging_root = release_dir.join("package");
 let staging_dir = staging_root.join(&package_name);

 let output_dir = match opts.output_dir {
  Some(p) => p,
  None => repo_root.join("release-packages"),
 };
 fs::create_dir_all(&output_dir).with_context(|| format!("failed to create output directory: {}", output_dir.display()))?;
 let output_dir = output_dir
  .canonicalize()
  .with_context(|| format!("failed to resolve output directory: {}", output_dir.display()))?;
 let zip_path = output_dir.join(format!("{}.zip", package_name));

 if !opts.skip_build {
  run_cargo_build(&repo_root)?;
 }

 if staging_dir.exists() {
  fs::remove_dir_all(&staging_dir).with_context(|| format!("failed to clean staging dir: {}", staging_dir.display()))?;
 }
 fs::create_dir_all(&staging_dir).with_context(|| format!("failed to create staging dir: {}", staging_dir.display()))?;

 let mut missing: Vec<PathBuf> = Vec::new();
 for file_name in REQUIRED_RELEASE_FILES {
  let source = release_dir.join(file_name);
  if !source.exists() {
   missing.push(source);
   continue;
  }
  fs::copy(&source, staging_dir.join(file_name)).with_context(|| format!("failed to copy required artifact: {}", source.display()))?;
 }

 if !missing.is_empty() {
  let joined = missing.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
  bail!("required release artifacts are missing:\n{}", joined);
 }

 for file_name in OPTIONAL_ROOT_FILES {
  let source = repo_root.join(file_name);
  if source.exists() {
   fs::copy(&source, staging_dir.join(file_name)).with_context(|| format!("failed to copy optional artifact: {}", source.display()))?;
  }
 }

 if zip_path.exists() {
  fs::remove_file(&zip_path).with_context(|| format!("failed to remove existing zip: {}", zip_path.display()))?;
 }

 create_zip(&staging_root, &package_name, &zip_path)?;

 if !opts.keep_staging {
  fs::remove_dir_all(&staging_dir).with_context(|| format!("failed to remove staging dir: {}", staging_dir.display()))?;

  if staging_root.exists() && fs::read_dir(&staging_root)?.next().is_none() {
   fs::remove_dir(&staging_root).with_context(|| format!("failed to remove empty staging root: {}", staging_root.display()))?;
  }
 }

 let metadata = fs::metadata(&zip_path).with_context(|| format!("failed to stat zip: {}", zip_path.display()))?;
 println!("Release package created:");
 println!("- Path: {}", zip_path.display());
 println!("- Size: {} bytes", metadata.len());
 if let Ok(modified) = metadata.modified() {
  if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
   println!("- Updated: {} (epoch seconds)", since_epoch.as_secs());
  }
 }
 println!("PACKAGE_PATH={}", zip_path.display());

 Ok(())
}

fn default_version(repo_root: &Path) -> Result<String> {
 let tauri_config_path = repo_root.join("apps/desktop/src-tauri/tauri.conf.json");
 if !tauri_config_path.exists() {
  bail!("tauri config not found: {}", tauri_config_path.display());
 }
 let raw =
  fs::read_to_string(&tauri_config_path).with_context(|| format!("failed to read tauri config: {}", tauri_config_path.display()))?;
 let value: serde_json::Value =
  serde_json::from_str(&raw).with_context(|| format!("failed to parse tauri config: {}", tauri_config_path.display()))?;
 let version = value
  .get("version")
  .and_then(|v| v.as_str())
  .map(|s| s.trim())
  .filter(|s| !s.is_empty())
  .ok_or_else(|| anyhow!("version is missing in {}", tauri_config_path.display()))?;
 Ok(version.to_string())
}

fn run_cargo_build(repo_root: &Path) -> Result<()> {
 let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
 let status = Command::new(&cargo)
  .current_dir(repo_root)
  .args(["build", "--release", "-p", "unvet-desktop", "-p", "unvet-cli"])
  .status()
  .with_context(|| "failed to invoke cargo build")?;
 if !status.success() {
  bail!("cargo build failed with exit code {}", status.code().unwrap_or(-1));
 }
 Ok(())
}

fn create_zip(staging_root: &Path, package_name: &str, zip_path: &Path) -> Result<()> {
 let package_dir = staging_root.join(package_name);
 let file = File::create(zip_path).with_context(|| format!("failed to create zip: {}", zip_path.display()))?;
 let mut writer = zip::ZipWriter::new(BufWriter::new(file));
 let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

 for entry in WalkDir::new(&package_dir).into_iter().filter_map(|e| e.ok()) {
  let path = entry.path();
  // Compute zip-relative path including the top-level package directory, mirroring `Compress-Archive`.
  let rel = path
   .strip_prefix(staging_root)
   .with_context(|| format!("failed to compute relative path for {}", path.display()))?;
  // zip uses forward slashes regardless of platform.
  let name = rel
   .components()
   .map(|c| c.as_os_str().to_string_lossy().into_owned())
   .collect::<Vec<_>>()
   .join("/");

  if entry.file_type().is_dir() {
   if name.is_empty() {
    continue;
   }
   writer
    .add_directory(format!("{}/", name), options)
    .with_context(|| format!("failed to add directory entry: {}", name))?;
  } else if entry.file_type().is_file() {
   writer
    .start_file(&name, options)
    .with_context(|| format!("failed to start zip entry: {}", name))?;
   let mut reader = BufReader::new(File::open(path).with_context(|| format!("failed to open {}", path.display()))?);
   let mut buf = [0u8; COPY_BUFFER_SIZE];
   loop {
    let n = reader.read(&mut buf)?;
    if n == 0 {
     break;
    }
    writer.write_all(&buf[..n])?;
   }
  }
 }

 writer.finish().with_context(|| "failed to finalize zip archive")?;
 Ok(())
}
