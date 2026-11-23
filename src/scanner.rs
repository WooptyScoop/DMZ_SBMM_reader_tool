use crate::model::{HitFile, ScanResult, ScanSummary, now_utc_iso};
use regex::{Regex, RegexBuilder};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub paths: Vec<PathBuf>,
    pub output: PathBuf,
    pub json_output: Option<PathBuf>,
    pub zip_output: Option<PathBuf>,
    pub max_file_mb: u64,
    pub no_size_limit: bool,
    pub skip_binary: bool,
    pub keywords: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            output: PathBuf::from("DMZ_SBMM_Reader_result.txt"),
            json_output: None,
            zip_output: None,
            max_file_mb: 32,
            no_size_limit: false,
            skip_binary: false,
            keywords: default_keywords(),
        }
    }
}

pub fn default_keywords() -> Vec<String> {
    [
        "sbmm",
        "skill based",
        "skill-based",
        "skill_based",
        "matchmaking",
        "mmr",
        "elo",
        "skill rating",
        "skill bracket",
        "meld",
        "dmz",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub fn detect_cod_paths() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(up) = std::env::var_os("USERPROFILE") {
        let up = PathBuf::from(up);
        candidates.push(up.join("Documents").join("Call of Duty"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local = PathBuf::from(local);
        candidates.push(local.join("Activision").join("Call of Duty"));
        candidates.push(local.join("Call of Duty"));
    }
    if let Some(pd) = std::env::var_os("ProgramData") {
        candidates.push(PathBuf::from(pd).join("Call of Duty"));
    }

    for pf in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(root) = std::env::var_os(pf) {
            let steam = PathBuf::from(root).join("Steam").join("steamapps").join("common");
            if steam.is_dir() && let Ok(rd) = fs::read_dir(&steam) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        if name.to_ascii_lowercase().contains("call of duty") {
                            candidates.push(p);
                        }
                    }
                }
            }
        }
    }

    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let d = PathBuf::from(&drive);
        if !d.exists() { continue; }
        for suffix in [
            "XboxGames\\Call of Duty",
            "xbox games\\Call of Duty",
            "Program Files\\Call of Duty",
            "Program Files (x86)\\Call of Duty",
            "Call of Duty",
            "SteamLibrary\\steamapps\\common\\Call of Duty",
            "Steam\\steamapps\\common\\Call of Duty",
        ] {
            candidates.push(d.join(suffix));
        }
    }

    for c in candidates {
        if c.exists() && !found.iter().any(|p| p == &c) {
            found.push(c);
        }
    }
    found
}

fn build_keyword_regex(keywords: &[String]) -> Result<Regex, String> {
    let mut parts = Vec::new();
    for k in keywords {
        let escaped = regex::escape(k);
        if k.len() <= 4 {
            parts.push(format!(r"\b{}\b", escaped));
        } else {
            parts.push(escaped);
        }
    }
    RegexBuilder::new(&parts.join("|"))
        .case_insensitive(true)
        .build()
        .map_err(|e| e.to_string())
}



pub fn scan(opts: &ScanOptions) -> Result<ScanResult, String> {
    let started = now_utc_iso();
    let rx = build_keyword_regex(&opts.keywords)?;
    let max_bytes: u64 = if opts.no_size_limit { u64::MAX } else { opts.max_file_mb.saturating_mul(1024 * 1024) };
    let text_exts = ["txt", "json", "xml", "cfg", "ini", "log", "csv"];

    let mut lines: Vec<String> = Vec::new();
    lines.push("=== DMZ SBMM READER - READ-ONLY FILE SCAN ===".to_string());
    lines.push(format!("Started: {started}"));
    lines.push("Safety: file reads only. No process APIs, no network, no memory reads, no injection, no writes to scanned folders.".to_string());
    lines.push("Note: no matches does NOT prove SBMM is absent; matchmaking is server-side.".to_string());
    if opts.no_size_limit {
        lines.push("Max file size: unlimited".to_string());
    } else {
        lines.push(format!("Max file size: {} MB", opts.max_file_mb));
    }
    lines.push(format!("Skip binary: {}", opts.skip_binary));
    lines.push(format!("Keywords: {}", opts.keywords.join(", ")));
    lines.push(String::new());
    lines.push("=== PATHS ===".to_string());
    if opts.paths.is_empty() {
        lines.push("PATH: (none found - specify folders to scan)".to_string());
    } else {
        for p in &opts.paths { lines.push(format!("PATH: {}", p.display())); }
    }
    lines.push(String::new());

    let mut scanned: usize = 0;
    let mut skipped: usize = 0;
    let mut errors: usize = 0;
    let mut hits_total: usize = 0;
    let mut hit_files: Vec<HitFile> = Vec::new();

    for root in &opts.paths {
        if !root.exists() {
            lines.push(format!("ROOT MISSING: {}", root.display()));
            continue;
        }
        lines.push(format!("ROOT: {}", root.display()));
        for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() { continue; }
            let path = entry.path();
            let meta = match entry.metadata() { Ok(m) => m, Err(_) => { errors += 1; continue; } };
            let len = meta.len();
            if !opts.no_size_limit && len > max_bytes { skipped += 1; continue; }
            scanned += 1;

            let mut file_hits: usize = 0;
            let mut samples: Vec<String> = Vec::new();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();

            let scan_result: Result<(), String> = (|| {
                if text_exts.iter().any(|e| *e == ext) {
                    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
                    for m in rx.find_iter(&text) {
                        file_hits += 1;
                        if samples.len() < 8 {
                            let start = m.start().saturating_sub(80);
                            let end = (m.end() + 100).min(text.len());
                            // ensure char boundaries
                            let start = text.floor_char_boundary(start);
                            let end = text.ceil_char_boundary(end);
                            let sample = text[start..end].split_whitespace().collect::<Vec<_>>().join(" ");
                            samples.push(format!("TEXT: {sample}"));
                        }
                    }
                } else if !opts.skip_binary {
                    let mut bytes = Vec::new();
                    fs::File::open(path).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
                    // Extract printable ASCII runs
                    let mut run: Vec<u8> = Vec::new();
                    let mut runs: Vec<String> = Vec::new();
                    for &b in &bytes {
                        if (0x20..=0x7e).contains(&b) {
                            run.push(b);
                        } else {
                            if run.len() >= 5 && let Ok(s) = String::from_utf8(run.clone()) { runs.push(s); }
                            run.clear();
                        }
                    }
                    if run.len() >= 5 && let Ok(s) = String::from_utf8(run) { runs.push(s); }
                    for s in runs {
                        if rx.is_match(&s) {
                            file_hits += 1;
                            if samples.len() < 8 {
                                samples.push(format!("ASCII: {s}"));
                            }
                        }
                    }
                }
                Ok(())
            })();

            if let Err(e) = scan_result {
                errors += 1;
                samples.push(format!("READ_ERROR: {e}"));
            }

            if file_hits > 0 {
                hits_total += file_hits;
                let modified = meta.modified().ok()
                    .map(chrono::DateTime::<chrono::Utc>::from)
                    .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                    .unwrap_or_default();
                lines.push(format!("FILE path={} size={} modifiedUtc={} hits={}", path.display(), len, modified, file_hits));
                for s in &samples { lines.push(format!("  {s}")); }
                hit_files.push(HitFile {
                    path: path.display().to_string(),
                    size: len,
                    modified_utc: modified,
                    hits: file_hits,
                    samples: samples.clone(),
                });
            }
        }
        lines.push(String::new());
    }

    lines.push("=== SUMMARY ===".to_string());
    lines.push(format!("Files scanned: {scanned}"));
    if opts.no_size_limit {
        lines.push("Files skipped (size limit): 0 (unlimited)".to_string());
    } else {
        lines.push(format!("Files skipped (>{} MB): {skipped}", opts.max_file_mb));
    }
    lines.push(format!("Read errors: {errors}"));
    lines.push(format!("Total keyword hits: {hits_total}"));
    let finished = now_utc_iso();
    lines.push(format!("Finished: {finished}"));

    if let Some(parent) = opts.output.parent() { let _ = fs::create_dir_all(parent); }
    fs::write(&opts.output, lines.join("\n")).map_err(|e| e.to_string())?;

    let result = ScanResult {
        started,
        finished: finished.clone(),
        paths: opts.paths.iter().map(|p| p.display().to_string()).collect(),
        output: opts.output.display().to_string(),
        json_output: opts.json_output.as_ref().map(|p| p.display().to_string()),
        zip_output: opts.zip_output.as_ref().map(|p| p.display().to_string()),
        no_size_limit: opts.no_size_limit,
        max_file_mb: if opts.no_size_limit { None } else { Some(opts.max_file_mb) },
        skip_binary: opts.skip_binary,
        keywords: opts.keywords.clone(),
        summary: ScanSummary {
            scanned, skipped, errors, hits: hits_total, hit_files: hit_files.len(),
        },
        hit_files,
    };

    if let Some(json_path) = &opts.json_output {
        if let Some(parent) = json_path.parent() { let _ = fs::create_dir_all(parent); }
        let json = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
        fs::write(json_path, json).map_err(|e| e.to_string())?;
    }

    if let Some(zip_path) = &opts.zip_output {
        let mut files = vec![opts.output.clone()];
        if let Some(j) = &opts.json_output && j.exists() { files.push(j.clone()); }
        crate::zip_export::zip_files(&files, zip_path)?;
    }

    Ok(result)
}

