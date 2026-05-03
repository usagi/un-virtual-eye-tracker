use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Options {
 pub repo_root: PathBuf,
 pub version: String,
 pub dry_run: bool,
 pub tag: bool,
}

struct PendingUpdate {
 rel_path: String,
 content: String,
}

const TOML_VERSION_FILES: &[&str] = &["Cargo.toml", "apps/desktop/src-tauri/Cargo.toml"];
const JSON_VERSION_FILES: &[&str] = &[
 "apps/desktop/package.json",
 "apps/desktop/package-lock.json",
 "apps/desktop/src-tauri/tauri.conf.json",
];
const LOCK_FILES: &[&str] = &["Cargo.lock", "apps/desktop/src-tauri/Cargo.lock"];
const LOCAL_LOCK_PACKAGE_NAMES: &[&str] = &[
 "unvet-cli",
 "unvet-config",
 "unvet-core",
 "unvet-desktop",
 "unvet-input-ifacialmocap",
 "unvet-input-vmc-osc",
 "unvet-output",
 "unvet-output-ets2",
 "unvet-output-keyboard",
 "unvet-output-mouse",
 "unvet-output-touch",
 "unvet-ui",
 "xtask",
];

pub fn run(opts: Options) -> Result<()> {
 validate_version(&opts.version)?;

 let repo_root = opts
  .repo_root
  .canonicalize()
  .with_context(|| format!("failed to resolve repository root: {}", opts.repo_root.display()))?;

 let local_package_names = LOCAL_LOCK_PACKAGE_NAMES.iter().copied().collect::<BTreeSet<_>>();
 let mut updates = Vec::new();

 for rel_path in TOML_VERSION_FILES {
  collect_update(&repo_root, rel_path, &mut updates, |raw| update_toml_version(raw, &opts.version))?;
 }

 for rel_path in JSON_VERSION_FILES {
  let max_replacements = if *rel_path == "apps/desktop/package-lock.json" { 2 } else { 1 };
  collect_update(&repo_root, rel_path, &mut updates, |raw| {
   update_json_version(raw, &opts.version, max_replacements)
  })?;
 }

 for rel_path in LOCK_FILES {
  collect_update(&repo_root, rel_path, &mut updates, |raw| {
   update_cargo_lock_versions(raw, &opts.version, &local_package_names)
  })?;
 }

 if opts.tag && !updates.is_empty() && !opts.dry_run {
  bail!("refusing to create a git tag while version files still need updates; run without --tag, commit the bump, then run with --tag");
 }

 let updated = updates.iter().map(|update| update.rel_path.clone()).collect::<Vec<_>>();
 if !opts.dry_run {
  for update in updates {
   let path = repo_root.join(&update.rel_path);
   fs::write(&path, update.content).with_context(|| format!("failed to write {}", path.display()))?;
  }
 }

 if opts.dry_run {
  println!("Version bump dry run: {}", opts.version);
 } else {
  println!("Version bumped to {}", opts.version);
 }

 if updated.is_empty() {
  println!("- No files needed changes");
 } else {
  for rel_path in updated {
   println!("- {}", rel_path);
  }
 }

 if opts.tag {
  let tag_name = format!("v{}", opts.version);
  if opts.dry_run {
   println!("- Would create git tag: {}", tag_name);
   println!("- Would run: git push --tags");
  } else {
   create_and_push_tag(&repo_root, &tag_name)?;
  }
 }

 Ok(())
}

fn collect_update<F>(repo_root: &Path, rel_path: &str, updates: &mut Vec<PendingUpdate>, updater: F) -> Result<()>
where
 F: FnOnce(&str) -> Result<String>,
{
 let path = repo_root.join(rel_path);
 let raw = fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
 let next = updater(&raw).with_context(|| format!("failed to update {}", rel_path))?;

 if next == raw {
  return Ok(());
 }

 updates.push(PendingUpdate {
  rel_path: rel_path.to_string(),
  content: next,
 });

 Ok(())
}

fn create_and_push_tag(repo_root: &Path, tag_name: &str) -> Result<()> {
 ensure_git_worktree_clean(repo_root)?;
 run_git(repo_root, &["tag", tag_name])?;
 run_git(repo_root, &["push", "--tags"])?;
 println!("- Created git tag: {}", tag_name);
 println!("- Pushed git tags");
 Ok(())
}

fn ensure_git_worktree_clean(repo_root: &Path) -> Result<()> {
 let output = Command::new("git")
  .current_dir(repo_root)
  .args(["status", "--porcelain"])
  .output()
  .with_context(|| "failed to invoke git status")?;
 if !output.status.success() {
  bail!("git status failed with exit code {}", output.status.code().unwrap_or(-1));
 }

 let status = String::from_utf8_lossy(&output.stdout);
 if !status.trim().is_empty() {
  bail!("refusing to create a git tag while the working tree is dirty")
 }

 Ok(())
}

fn run_git(repo_root: &Path, args: &[&str]) -> Result<()> {
 let status = Command::new("git")
  .current_dir(repo_root)
  .args(args)
  .status()
  .with_context(|| format!("failed to invoke git {}", args.join(" ")))?;
 if !status.success() {
  bail!("git {} failed with exit code {}", args.join(" "), status.code().unwrap_or(-1));
 }
 Ok(())
}

fn validate_version(version: &str) -> Result<()> {
 let version_re = Regex::new(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$")?;
 if version_re.is_match(version) {
  Ok(())
 } else {
  bail!("version must be SemVer-like MAJOR.MINOR.PATCH, got: {}", version)
 }
}

fn update_toml_version(raw: &str, version: &str) -> Result<String> {
 let version_re = Regex::new(r#"(?m)^(version(?:\.workspace)?\s*=\s*)"[^"]+""#)?;
 let mut replacements = 0;
 let next = version_re
  .replace_all(raw, |captures: &regex::Captures<'_>| {
   replacements += 1;
   format!(r#"{}"{}""#, &captures[1], version)
  })
  .into_owned();

 if replacements == 0 {
  bail!("no package version assignment found")
 }
 Ok(next)
}

fn update_json_version(raw: &str, version: &str, max_replacements: usize) -> Result<String> {
 let _: Value = serde_json::from_str(raw).with_context(|| "invalid JSON before version update")?;
 let version_re = Regex::new(r#"("version"\s*:\s*)"[^"]+""#)?;
 let mut replacements = 0;
 let next = version_re
  .replace_all(raw, |captures: &regex::Captures<'_>| {
   if replacements >= max_replacements {
    return captures[0].to_string();
   }
   replacements += 1;
   format!(r#"{}"{}""#, &captures[1], version)
  })
  .into_owned();

 if replacements == 0 {
  bail!("no JSON version property found")
 }
 let _: Value = serde_json::from_str(&next).with_context(|| "invalid JSON after version update")?;
 Ok(next)
}

fn update_cargo_lock_versions(raw: &str, version: &str, local_package_names: &BTreeSet<&str>) -> Result<String> {
 let mut next = String::with_capacity(raw.len());
 let mut current_package: Option<String> = None;
 let mut changed_packages = 0;
 let name_re = Regex::new(r#"^name = "([^"]+)"$"#)?;
 let version_re = Regex::new(r#"^version = "[^"]+"$"#)?;

 for line in raw.lines() {
  let mut next_line = line.to_string();

  if line == "[[package]]" {
   current_package = None;
  } else if let Some(captures) = name_re.captures(line) {
   current_package = Some(captures[1].to_string());
  } else if version_re.is_match(line)
   && current_package
    .as_deref()
    .is_some_and(|package_name| local_package_names.contains(package_name))
  {
   next_line = format!(r#"version = "{}""#, version);
   changed_packages += 1;
  }

  next.push_str(&next_line);
  next.push('\n');
 }

 if !raw.ends_with('\n') {
  next.pop();
 }

 if changed_packages == 0 {
  return Err(anyhow!("no local package versions found in lockfile"));
 }

 Ok(next)
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn cargo_lock_update_only_touches_local_packages() {
  let local_package_names = LOCAL_LOCK_PACKAGE_NAMES.iter().copied().collect::<BTreeSet<_>>();
  let raw = r#"[[package]]
name = "http"
version = "1.4.0"

[[package]]
name = "unvet-core"
version = "1.4.0"
"#;

  let updated = update_cargo_lock_versions(raw, "1.4.1", &local_package_names).unwrap();

  assert!(updated.contains("name = \"http\"\nversion = \"1.4.0\""));
  assert!(updated.contains("name = \"unvet-core\"\nversion = \"1.4.1\""));
 }

 #[test]
 fn version_validation_accepts_semver_prerelease() {
  validate_version("1.4.1-beta.1+build.5").unwrap();
 }

 #[test]
 fn version_validation_rejects_partial_versions() {
  assert!(validate_version("1.4").is_err());
 }

 #[test]
 fn package_lock_update_keeps_dependency_versions() {
  let raw = r#"{
 "version": "1.4.0",
 "packages": {
  "": {
   "version": "1.4.0"
  },
  "node_modules/example": {
   "version": "9.9.9"
  }
 }
}
"#;

  let updated = update_json_version(raw, "1.4.1", 2).unwrap();

  assert!(updated.contains("\"version\": \"1.4.1\""));
  assert!(updated.contains("\"version\": \"9.9.9\""));
 }
}
