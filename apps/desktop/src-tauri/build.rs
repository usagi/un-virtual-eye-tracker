use std::{env, path::PathBuf, process::Command};

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

 build_frontend_dist();

 let attrs = tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path("icons/icon.ico"));
 tauri_build::try_build(attrs).expect("failed to run tauri-build");
}

fn build_frontend_dist() {
 if env::var("UNVET_SKIP_FRONTEND_BUILD").ok().as_deref() == Some("1") {
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
