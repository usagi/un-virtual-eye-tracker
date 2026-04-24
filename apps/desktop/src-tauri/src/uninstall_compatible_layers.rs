use std::{env, fs, path::PathBuf, process::Command};

const TRACKIR_INSTALLER_REG_VALUE: &str = "Path";
const TRACKIR_REGISTRY_KEYS: [&str; 6] = [
 r"Software\NaturalPoint\NATURALPOINT\NPClient Location",
 r"Software\NaturalPoint\NATURALPOINT\NPClient64 Location",
 r"Software\NaturalPoint\NaturalPoint\NPClient Location",
 r"Software\NaturalPoint\NaturalPoint\NPClient64 Location",
 r"Software\Freetrack\FreeTrackClient",
 r"Software\Freetrack\FreetrackClient",
];
const COMPATIBILITY_FILES: [&str; 3] = ["NPClient64.dll", "NPClient.dll", "TrackIR.exe"];

#[derive(Debug, Default)]
struct CliOptions {
 dry_run: bool,
 help: bool,
}

fn main() {
 if cfg!(not(target_os = "windows")) {
  eprintln!("This tool is supported only on Windows.");
  std::process::exit(2);
 }

 let options = parse_args();
 if options.help {
  print_help();
  return;
 }

 let executable_dir = match resolve_executable_dir() {
  Ok(dir) => dir,
  Err(error) => {
   eprintln!("Failed to resolve executable directory: {error}");
   std::process::exit(1);
  },
 };

 let target_dir = normalize_registry_dir_value(&executable_dir);
 println!("[unvet-uninstall-compatible-layers] target directory: {}", executable_dir.display());
 if options.dry_run {
  println!("[unvet-uninstall-compatible-layers] dry-run mode enabled; no changes will be made.");
 }

 let mut had_error = false;
 let mut removed_registry_keys = 0usize;
 let mut skipped_registry_keys = 0usize;
 for key_path in TRACKIR_REGISTRY_KEYS {
  match uninstall_registry_key_if_owned(key_path, &target_dir, options.dry_run) {
   Ok(true) => {
    removed_registry_keys += 1;
   },
   Ok(false) => {
    skipped_registry_keys += 1;
   },
   Err(error) => {
    had_error = true;
    eprintln!("[registry] {key_path}: {error}");
   },
  }
 }

 let mut removed_files = 0usize;
 let mut missing_files = 0usize;
 for file_name in COMPATIBILITY_FILES {
  let path = executable_dir.join(file_name);
  if !path.exists() {
   missing_files += 1;
   println!("[file] not found, skipping: {}", path.display());
   continue;
  }

  if options.dry_run {
   removed_files += 1;
   println!("[file] would remove: {}", path.display());
   continue;
  }

  match fs::remove_file(&path) {
   Ok(()) => {
    removed_files += 1;
    println!("[file] removed: {}", path.display());
   },
   Err(error) => {
    had_error = true;
    eprintln!("[file] failed to remove {}: {error}", path.display());
   },
  }
 }

 println!(
  "[summary] registry removed: {removed_registry_keys}, registry skipped: {skipped_registry_keys}, files removed: {removed_files}, files missing: {missing_files}"
 );

 if had_error {
  eprintln!("[summary] completed with errors.");
  std::process::exit(1);
 }

 println!("[summary] completed successfully.");
}

fn parse_args() -> CliOptions {
 let mut options = CliOptions::default();

 for arg in env::args().skip(1) {
  match arg.as_str() {
   "--dry-run" => options.dry_run = true,
   "--help" | "-h" | "/?" => options.help = true,
   _ => {
    eprintln!("Unknown argument: {arg}");
    print_help();
    std::process::exit(2);
   },
  }
 }

 options
}

fn print_help() {
 println!("unvet-uninstall-compatible-layers.exe [--dry-run]");
 println!();
 println!("Removes UNVET NPClient/TrackIR compatibility layers from the current installation directory.");
 println!("Only registry keys that currently point to this directory are removed.");
}

fn resolve_executable_dir() -> Result<PathBuf, String> {
 let exe_path = env::current_exe().map_err(|error| error.to_string())?;
 let dir = exe_path
  .parent()
  .ok_or_else(|| "executable parent directory is missing".to_owned())?
  .to_path_buf();
 Ok(dir)
}

fn normalize_registry_dir_value(path: &PathBuf) -> String {
 let mut value = path.to_string_lossy().to_string();
 if !value.ends_with('\\') {
  value.push('\\');
 }
 value
}

fn uninstall_registry_key_if_owned(key_path: &str, target_dir: &str, dry_run: bool) -> Result<bool, String> {
 let full_key = format!(r"HKCU\{key_path}");
 if !registry_key_exists(&full_key) {
  println!("[registry] key not found, skipping: {full_key}");
  return Ok(false);
 }

 let default_value = query_registry_value(&full_key, None)?;
 let path_value = query_registry_value(&full_key, Some(TRACKIR_INSTALLER_REG_VALUE))?;
 let path_subkey = format!(r"{full_key}\{TRACKIR_INSTALLER_REG_VALUE}");
 let path_sub_default = query_registry_value(&path_subkey, None)?;

 let owned_by_target = [default_value, path_value, path_sub_default]
  .into_iter()
  .flatten()
  .any(|value| registry_value_matches_target(&value, target_dir));

 if !owned_by_target {
  println!("[registry] key does not point to this installation, skipping: {full_key}");
  return Ok(false);
 }

 if dry_run {
  println!("[registry] would remove key: {full_key}");
  return Ok(true);
 }

 delete_registry_key_recursive(&full_key)?;
 println!("[registry] removed key: {full_key}");
 Ok(true)
}

fn registry_key_exists(full_key: &str) -> bool {
 let output = Command::new("reg").arg("query").arg(full_key).output();
 matches!(output, Ok(result) if result.status.success())
}

fn query_registry_value(full_key: &str, value_name: Option<&str>) -> Result<Option<String>, String> {
 let mut command = Command::new("reg");
 command.arg("query").arg(full_key);

 match value_name {
  Some(name) => {
   command.arg("/v").arg(name);
  },
  None => {
   command.arg("/ve");
  },
 }

 let output = command
  .output()
  .map_err(|error| format!("failed to execute reg query for {full_key}: {error}"))?;

 if !output.status.success() {
  return Ok(None);
 }

 let stdout = String::from_utf8_lossy(&output.stdout);
 for line in stdout.lines() {
  if let Some(parsed) = parse_registry_query_line(line) {
   return Ok(Some(parsed));
  }
 }

 Ok(None)
}

fn parse_registry_query_line(line: &str) -> Option<String> {
 let trimmed = line.trim();
 if trimmed.is_empty() || trimmed.starts_with("HKEY_") {
  return None;
 }

 let mut parts = trimmed.split_whitespace();
 let _value_label = parts.next()?;
 let value_type = parts.next()?;
 if !value_type.starts_with("REG_") {
  return None;
 }

 let data = parts.collect::<Vec<_>>().join(" ");
 if data.is_empty() {
  None
 } else {
  Some(data)
 }
}

fn registry_value_matches_target(value: &str, target_dir: &str) -> bool {
 let normalize = |input: &str| -> String {
  let mut text = input.trim().replace('/', "\\");
  while text.ends_with(' ') {
   text.pop();
  }
  if !text.ends_with('\\') {
   text.push('\\');
  }
  text.make_ascii_lowercase();
  text
 };

 normalize(value) == normalize(target_dir)
}

fn delete_registry_key_recursive(full_key: &str) -> Result<(), String> {
 let output = Command::new("reg")
  .arg("delete")
  .arg(full_key)
  .arg("/f")
  .output()
  .map_err(|error| format!("failed to execute reg delete for {full_key}: {error}"))?;

 if output.status.success() {
  return Ok(());
 }

 let stderr = String::from_utf8_lossy(&output.stderr);
 let stdout = String::from_utf8_lossy(&output.stdout);
 Err(format!("reg delete failed for {full_key}: {} {}", stderr.trim(), stdout.trim()))
}
