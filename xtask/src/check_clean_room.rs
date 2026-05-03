use anyhow::{Context, Result};
use regex::RegexBuilder;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

const SCAN_ROOTS: &[&str] = &["crates", "apps", "config"];
const FILE_EXTENSIONS: &[&str] = &["rs", "toml", "ts", "js", "svelte", "c", "cpp", "h", "hpp"];

struct DenyRule {
 pattern: &'static str,
 reason: &'static str,
}

const DENY_LIST: &[DenyRule] = &[
 DenyRule {
  pattern: r"TrackIR SDK",
  reason: "proprietary SDK reference",
 },
 DenyRule {
  pattern: r"NaturalPoint SDK",
  reason: "proprietary SDK reference",
 },
 DenyRule {
  pattern: r#"#include\s*[<"]NPClient\.h[>"]"#,
  reason: "SDK header reference",
 },
 DenyRule {
  pattern: r"leaked\s+code|leak(ed)?\s+source",
  reason: "leak-derived source reference",
 },
 DenyRule {
  pattern: r"decompil(e|ed)|disassembl(y|ed)",
  reason: "reverse-engineering source reference",
 },
 DenyRule {
  pattern: r"private\s+internal\s+structure|proprietary\s+internal",
  reason: "private internal reference",
 },
];

struct Hit {
 file: String,
 line: usize,
 reason: &'static str,
 text: String,
}

pub fn run(repo_root: &Path) -> Result<()> {
 let repo_root = repo_root
  .canonicalize()
  .with_context(|| format!("failed to resolve repository root: {}", repo_root.display()))?;

 let rules: Vec<(regex::Regex, &'static str)> = DENY_LIST
  .iter()
  .map(|rule| {
   let regex = RegexBuilder::new(rule.pattern)
    .case_insensitive(true)
    .build()
    .with_context(|| format!("invalid deny-list pattern: {}", rule.pattern))?;
   Ok((regex, rule.reason))
  })
  .collect::<Result<_>>()?;

 let mut hits: Vec<Hit> = Vec::new();

 for root in SCAN_ROOTS {
  let full_root = repo_root.join(root);
  if !full_root.exists() {
   continue;
  }

  for entry in WalkDir::new(&full_root).into_iter().filter_map(|e| e.ok()) {
   if !entry.file_type().is_file() {
    continue;
   }
   let path = entry.path();
   let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
    continue;
   };
   let ext_lower = ext.to_ascii_lowercase();
   if !FILE_EXTENSIONS.iter().any(|e| *e == ext_lower) {
    continue;
   }

   let content = match fs::read_to_string(path) {
    Ok(c) => c,
    Err(_) => continue, // skip non-utf8 / unreadable files, mirroring the PowerShell behaviour
   };

   for (line_idx, line) in content.lines().enumerate() {
    for (regex, reason) in &rules {
     if regex.is_match(line) {
      hits.push(Hit {
       file: path.display().to_string(),
       line: line_idx + 1,
       reason,
       text: line.trim().to_string(),
      });
     }
    }
   }
  }
 }

 if !hits.is_empty() {
  eprintln!("Clean-room check failed:");
  for hit in &hits {
   eprintln!("- {}:{} [{}] {}", hit.file, hit.line, hit.reason, hit.text);
  }
  std::process::exit(1);
 }

 println!("Clean-room check passed.");
 Ok(())
}
