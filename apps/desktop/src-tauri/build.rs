use std::{env, fs, io, path::PathBuf, process::Command, thread, time::Duration};

fn main() {
 println!("cargo:rerun-if-env-changed=UNVET_SKIP_FRONTEND_BUILD");
 println!("cargo:rerun-if-changed=../src");
 println!("cargo:rerun-if-changed=../index.html");
 println!("cargo:rerun-if-changed=../package.json");
 println!("cargo:rerun-if-changed=../package-lock.json");
 println!("cargo:rerun-if-changed=../vite.config.ts");
 println!("cargo:rerun-if-changed=../svelte.config.js");
 println!("cargo:rerun-if-changed=../tsconfig.app.json");
 println!("cargo:rerun-if-changed=../tsconfig.node.json");
 println!("cargo:rerun-if-changed=src/npclient64_shim.rs");
 println!("cargo:rerun-if-changed=src/trackir_dummy.rs");
 println!("cargo:rerun-if-changed=src/uninstall_compatible_layers.rs");

 build_frontend_dist();
 build_npclient_shims();
 build_trackir_dummy();
 build_compatibility_layer_uninstaller();

 let attrs = tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path("icons/icon.ico"));
 tauri_build::try_build(attrs).expect("failed to run tauri-build");
}

fn build_frontend_dist() {
 if should_skip_frontend_build() {
  println!("cargo:warning=skipping frontend build because UNVET_SKIP_FRONTEND_BUILD=1 in CI");
  return;
 }

 let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is missing"));
 let frontend_dir = manifest_dir
  .parent()
  .expect("failed to resolve frontend directory from src-tauri")
  .to_path_buf();

 let npm_executable = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };
 let status = Command::new(npm_executable)
  .arg("run")
  .arg("build")
  .current_dir(frontend_dir)
  .status()
  .expect("failed to start frontend build command");

 if !status.success() {
  panic!("frontend build failed with status: {status}");
 }
}

fn should_skip_frontend_build() -> bool {
 let skip_requested = env::var("UNVET_SKIP_FRONTEND_BUILD").ok().as_deref() == Some("1");
 let running_in_ci = env::var("CI")
  .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
  .unwrap_or(false);

 skip_requested && running_in_ci
}

fn build_npclient_shims() {
 if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("windows") {
  return;
 }

 let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is missing"));
 let source_path = manifest_dir.join("src").join("npclient64_shim.rs");

 let workspace_root = manifest_dir
  .ancestors()
  .nth(3)
  .expect("failed to resolve workspace root from src-tauri")
  .to_path_buf();

 let target_root = env::var("CARGO_TARGET_DIR")
  .map(PathBuf::from)
  .unwrap_or_else(|_| workspace_root.join("target"));
 let profile = env::var("PROFILE").expect("PROFILE is missing");
 let output_dir = target_root.join(&profile);
 fs::create_dir_all(&output_dir).expect("failed to create target profile directory");

 let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
 let target = env::var("TARGET").expect("TARGET is missing");

 build_npclient_shim_artifact(&rustc, &target, &source_path, &output_dir, "NPClient64", "NPClient64.dll");
 build_npclient_shim_artifact(&rustc, &target, &source_path, &output_dir, "NPClient", "NPClient.dll");
}

fn build_npclient_shim_artifact(
 rustc: &str,
 target: &str,
 source_path: &PathBuf,
 output_dir: &PathBuf,
 crate_name: &str,
 output_name: &str,
) {
 let output_dll = output_dir.join(output_name);
 if !artifact_needs_rebuild(source_path, &output_dll) {
  return;
 }

 let temp_output_dll = output_dir.join(format!("{}.{}.tmp.dll", crate_name, std::process::id()));

 let status = Command::new(rustc)
  .arg("--crate-name")
  .arg(crate_name)
  .arg("--crate-type")
  .arg("cdylib")
  .arg("--edition=2021")
  .arg("--target")
  .arg(target)
  .arg(source_path)
  .arg("-C")
  .arg("opt-level=2")
  .arg("-o")
  .arg(&temp_output_dll)
  .status()
  .unwrap_or_else(|error| panic!("failed to start rustc for {output_name}: {error}"));

 if !status.success() {
  panic!("failed to build {output_name}: rustc exited with status {status}");
 }

 if let Err(error) = replace_artifact(&temp_output_dll, &output_dll) {
  panic!("failed to place {output_name} at {}: {error}", output_dll.display());
 }
}

fn build_trackir_dummy() {
 if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("windows") {
  return;
 }

 let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is missing"));
 let source_path = manifest_dir.join("src").join("trackir_dummy.rs");

 let workspace_root = manifest_dir
  .ancestors()
  .nth(3)
  .expect("failed to resolve workspace root from src-tauri")
  .to_path_buf();

 let target_root = env::var("CARGO_TARGET_DIR")
  .map(PathBuf::from)
  .unwrap_or_else(|_| workspace_root.join("target"));
 let profile = env::var("PROFILE").expect("PROFILE is missing");
 let output_dir = target_root.join(&profile);
 fs::create_dir_all(&output_dir).expect("failed to create target profile directory");

 let output_exe = output_dir.join("TrackIR.exe");
 if !artifact_needs_rebuild(&source_path, &output_exe) {
  return;
 }

 let temp_output_exe = output_dir.join(format!("TrackIR.{}.tmp.exe", std::process::id()));
 let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
 let target = env::var("TARGET").expect("TARGET is missing");

 let status = Command::new(rustc)
  .arg("--crate-name")
  .arg("TrackIR")
  .arg("--crate-type")
  .arg("bin")
  .arg("--edition=2021")
  .arg("--target")
  .arg(target)
  .arg(&source_path)
  .arg("-C")
  .arg("opt-level=2")
  .arg("-o")
  .arg(&temp_output_exe)
  .status()
  .expect("failed to start rustc for TrackIR dummy process");

 if !status.success() {
  panic!("failed to build TrackIR dummy process: rustc exited with status {status}");
 }

 if let Err(error) = replace_artifact(&temp_output_exe, &output_exe) {
  panic!("failed to place TrackIR dummy process at {}: {error}", output_exe.display());
 }
}

fn build_compatibility_layer_uninstaller() {
 if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("windows") {
  return;
 }

 let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is missing"));
 let source_path = manifest_dir.join("src").join("uninstall_compatible_layers.rs");

 let workspace_root = manifest_dir
  .ancestors()
  .nth(3)
  .expect("failed to resolve workspace root from src-tauri")
  .to_path_buf();

 let target_root = env::var("CARGO_TARGET_DIR")
  .map(PathBuf::from)
  .unwrap_or_else(|_| workspace_root.join("target"));
 let profile = env::var("PROFILE").expect("PROFILE is missing");
 let output_dir = target_root.join(&profile);
 fs::create_dir_all(&output_dir).expect("failed to create target profile directory");

 let output_exe = output_dir.join("unvet-uninstall-compatible-layers.exe");
 if !artifact_needs_rebuild(&source_path, &output_exe) {
  return;
 }

 let temp_output_exe = output_dir.join(format!("unvet-uninstall-compatible-layers.{}.tmp.exe", std::process::id()));
 let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
 let target = env::var("TARGET").expect("TARGET is missing");

 let status = Command::new(rustc)
  .arg("--crate-name")
  .arg("unvet_uninstall_compatible_layers")
  .arg("--crate-type")
  .arg("bin")
  .arg("--edition=2021")
  .arg("--target")
  .arg(target)
  .arg(&source_path)
  .arg("-C")
  .arg("opt-level=2")
  .arg("-o")
  .arg(&temp_output_exe)
  .status()
  .expect("failed to start rustc for compatibility layer uninstaller");

 if !status.success() {
  panic!("failed to build compatibility layer uninstaller: rustc exited with status {status}");
 }

 if let Err(error) = replace_artifact(&temp_output_exe, &output_exe) {
  panic!(
   "failed to place compatibility layer uninstaller at {}: {error}",
   output_exe.display()
  );
 }
}

fn artifact_needs_rebuild(source_path: &PathBuf, output_path: &PathBuf) -> bool {
 let source_modified = fs::metadata(source_path).and_then(|metadata| metadata.modified());
 let output_modified = fs::metadata(output_path).and_then(|metadata| metadata.modified());

 match (source_modified, output_modified) {
  (Ok(source_time), Ok(output_time)) => source_time > output_time,
  _ => true,
 }
}

fn replace_artifact(temp_path: &PathBuf, final_path: &PathBuf) -> io::Result<()> {
 const MAX_RETRIES: usize = 12;
 const RETRY_DELAY_MS: u64 = 120;

 let mut last_error: Option<io::Error> = None;

 for attempt in 0..=MAX_RETRIES {
  if final_path.exists() {
   let _ = fs::remove_file(final_path);
  }

  match fs::rename(temp_path, final_path) {
   Ok(()) => return Ok(()),
   Err(rename_error) => {
    last_error = Some(rename_error);
    if attempt < MAX_RETRIES {
     thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
    }
   },
  }
 }

 let rename_error = last_error.expect("rename should have produced an error");
 if !final_path.exists() {
  let _ = fs::remove_file(temp_path);
  return Err(rename_error);
 }

 println!(
  "cargo:warning=artifact is currently locked and cannot be replaced; keeping existing file at {} ({rename_error})",
  final_path.display()
 );
 let _ = fs::remove_file(temp_path);
 Ok(())
}
