use crate::model::{ProbePaths, now_local_hms, now_utc_iso};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Probe {
    pub paths: ProbePaths,
    pub scan_paths: Vec<PathBuf>,
    pub match_number: i32,
    pub queue_start: Option<chrono::DateTime<chrono::Local>>,
    pub baseline_done: bool,
}

impl Probe {
    pub fn new(out_dir: PathBuf, scan_paths: Vec<PathBuf>) -> Self {
        let paths = ProbePaths::new(out_dir);
        Self { paths, scan_paths, match_number: 1, queue_start: None, baseline_done: false }
    }

    pub fn init_files(&self) -> Result<(), String> {
        fs::create_dir_all(&self.paths.out_dir).map_err(|e| e.to_string())?;
        if !self.paths.tracker.exists() {
            fs::write(&self.paths.tracker, "Match#,QueueStartTime,QueueEndTime,QueueSeconds,PrevKills,PrevDeaths,PrevExfil,LobbyAvgLevel,LobbyAvgKD,Notes\n")
                .map_err(|e| e.to_string())?;
        }
        if !self.paths.network_csv.exists() {
            fs::write(&self.paths.network_csv, "TimestampUtc,ProcessName,PID,LocalAddress,LocalPort,RemoteAddress,RemotePort,State\n")
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn log(&self, msg: &str) {
        let line = format!("{}  {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.paths.log) {
            let _ = writeln!(f, "{line}");
        }
    }

    pub fn snapshot(&self, label: &str, light: bool) -> Result<PathBuf, String> {
        let out = self.paths.out_dir.join(format!("snapshot_{label}.txt"));
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("=== SNAPSHOT: {label} ==="));
        lines.push(format!("Time: {}", now_utc_iso()));
        lines.push(format!("Mode: {}", if light { "LIGHT" } else { "FULL" }));
        lines.push("Type: read-only process / network / file metadata".to_string());
        lines.push(String::new());

        lines.push("--- PROCESSES ---".to_string());
        // Read-only process/network metadata via PowerShell Get-NetTCPConnection/Get-Process on Windows.
        #[cfg(target_os = "windows")]
        {
            match collect_windows_process_conns() {
                Ok(pl) => lines.extend(pl),
                Err(e) => lines.push(format!("PROCESS_ERROR: {e}")),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            lines.push("(process/network snapshot only implemented for Windows in this build)".to_string());
        }
        lines.push(String::new());

        lines.push("--- FILES ---".to_string());
        for root in &self.scan_paths {
            if !root.exists() {
                lines.push(format!("ROOT MISSING: {}", root.display()));
                continue;
            }
            lines.push(format!("ROOT: {}", root.display()));
            for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok) {
                if !entry.file_type().is_file() { continue; }
                let path = entry.path();
                let meta = match entry.metadata() { Ok(m) => m, Err(_) => continue };
                let len = meta.len();
                if len >= 10 * 1024 * 1024 { continue; }
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                if light && !(name.contains("cod26") || name.contains("bootstrapper") || name.contains("ccs") || name.contains("secureattestation")) {
                    continue;
                }
                let modified = meta.modified().ok().map(chrono::DateTime::<chrono::Utc>::from)
                    .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default();
                let hash = if len < 1024 * 1024 {
                    sha256_file(path).unwrap_or_else(|_| "HASH_ERROR".to_string())
                } else { String::new() };
                lines.push(format!("FILE path={} size={} modifiedUtc={} sha256={}", path.display(), len, modified, hash));
            }
            lines.push(String::new());
        }

        fs::write(&out, lines.join("\n")).map_err(|e| e.to_string())?;
        self.log(&format!("Snapshot saved: {label}"));
        Ok(out)
    }

    pub fn sample_network(&self) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let rows = collect_windows_tcp_csv_rows()?;
            if rows.is_empty() { return Ok(()); }
            let mut f = OpenOptions::new().create(true).append(true).open(&self.paths.network_csv).map_err(|e| e.to_string())?;
            for r in rows { let _ = writeln!(f, "{r}"); }
        }
        Ok(())
    }

    pub fn mark_queue_start(&mut self) -> Result<String, String> {
        if !self.baseline_done {
            self.log("Taking baseline snapshot. This can take a moment...");
            self.snapshot("baseline", false)?;
            self.baseline_done = true;
        }
        self.queue_start = Some(chrono::Local::now());
        let t = self.queue_start.unwrap().to_rfc3339();
        self.log(&format!("MATCH {} QUEUE START: {t}", self.match_number));
        self.snapshot(&format!("m{:02}_queue", self.match_number), true)?;
        Ok(now_local_hms())
    }

    pub fn mark_queue_end(&mut self) -> Result<f64, String> {
        let Some(start) = self.queue_start.take() else { return Err("No queue start recorded".to_string()); };
        let end = chrono::Local::now();
        let seconds = (end - start).num_milliseconds() as f64 / 1000.0;
        self.log(&format!("MATCH {} MATCH FOUND: {}  (queue {:.1}s)", self.match_number, end.to_rfc3339(), seconds));
        self.snapshot(&format!("m{:02}_queue_end", self.match_number), true)?;
        self.update_tracker_row(self.match_number, Some(start), Some(end), seconds)?;
        Ok(seconds)
    }

    pub fn mark_match_end(&mut self) -> Result<(), String> {
        if self.queue_start.take().is_some() {
            self.log("Note: \"2. Match found\" was not pressed; queue end time was not recorded.");
        }
        self.log(&format!("MATCH {} MATCH END: {}", self.match_number, now_utc_iso()));
        self.snapshot(&format!("m{:02}_after", self.match_number), false)?;
        self.match_number += 1;
        Ok(())
    }

    fn update_tracker_row(&self, match_num: i32, start: Option<chrono::DateTime<chrono::Local>>, end: Option<chrono::DateTime<chrono::Local>>, seconds: f64) -> Result<(), String> {
        let content = fs::read_to_string(&self.paths.tracker).unwrap_or_default();
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push("Match#,QueueStartTime,QueueEndTime,QueueSeconds,PrevKills,PrevDeaths,PrevExfil,LobbyAvgLevel,LobbyAvgKD,Notes".to_string());
        }
        let prefix = format!("{match_num},");
        let mut found = false;
        for line in lines.iter_mut().skip(1) {
            if line.starts_with(&prefix) {
                let mut cols = parse_csv_simple(line);
                while cols.len() < 10 { cols.push(String::new()); }
                if let Some(s) = start { cols[1] = s.format("%H:%M:%S").to_string(); }
                if let Some(e) = end { cols[2] = e.format("%H:%M:%S").to_string(); }
                cols[3] = format!("{:.1}", seconds);
                *line = cols.join(",");
                found = true;
                break;
            }
        }
        if !found {
            let mut cols = vec![String::new(); 10];
            cols[0] = match_num.to_string();
            if let Some(s) = start { cols[1] = s.format("%H:%M:%S").to_string(); }
            if let Some(e) = end { cols[2] = e.format("%H:%M:%S").to_string(); }
            cols[3] = format!("{:.1}", seconds);
            lines.push(cols.join(","));
        }
        fs::write(&self.paths.tracker, lines.join("\n") + "\n").map_err(|e| e.to_string())
    }

    pub fn write_report(&self) -> Result<PathBuf, String> {
        let mut lines: Vec<String> = Vec::new();
        lines.push("=== DMZ SBMM PROBE REPORT ===".to_string());
        lines.push(format!("Generated: {}", now_utc_iso()));
        lines.push("All data below is read-only metadata. This tool does not inject into or modify the game.".to_string());
        lines.push(String::new());

        let tracker = fs::read_to_string(&self.paths.tracker).unwrap_or_default();
        let net = fs::read_to_string(&self.paths.network_csv).unwrap_or_default();
        let net_rows: Vec<Vec<String>> = net.lines().skip(1).map(parse_csv_simple).collect();
        let baseline = self.paths.out_dir.join("snapshot_baseline.txt");

        let mut count = 0;
        for row in tracker.lines().skip(1) {
            let cols = parse_csv_simple(row);
            if cols.is_empty() || cols[0].trim().is_empty() { continue; }
            let mnum: i32 = cols[0].trim().parse().unwrap_or(0);
            if mnum < 1 { continue; }
            count += 1;
            let qstart = cols.get(1).cloned().unwrap_or_default();
            let qend = cols.get(2).cloned().unwrap_or_default();
            let qsec = cols.get(3).cloned().unwrap_or_default();
            lines.push(format!("=== MATCH {mnum} ==="));
            lines.push(format!("Queue start: {qstart} | Queue end: {qend} | Queue seconds: {qsec}"));

            if !qstart.is_empty() && !qend.is_empty() {
                // Best-effort: compare HH:MM:SS as local today UTC for window - not perfect but reasonable.
                let endpoints = queue_window_endpoints(&net_rows, &qstart, &qend);
                let samples = endpoints.0;
                let eps = endpoints.1;
                lines.push(format!("Queue TCP samples: {samples}"));
                lines.push(format!("Queue TCP endpoints: {}", if eps.is_empty() { "(none)".to_string() } else { eps.join(", ") }));
            } else {
                lines.push("Queue TCP endpoints: (no window recorded)".to_string());
            }

            let queue_snap = self.paths.out_dir.join(format!("snapshot_m{mnum:02}_queue.txt"));
            if queue_snap.exists() {
                let snap = fs::read_to_string(&queue_snap).unwrap_or_default();
                let procs: Vec<&str> = snap.lines().filter(|l| l.starts_with("PROC ")).collect();
                lines.push(format!("Queue processes: {}", if procs.is_empty() { "(none recorded)".to_string() } else { procs.join(" | ") }));
            }

            let after_snap = self.paths.out_dir.join(format!("snapshot_m{mnum:02}_after.txt"));
            if after_snap.exists() && baseline.exists() {
                let b = fs::read_to_string(&baseline).unwrap_or_default();
                let a = fs::read_to_string(&after_snap).unwrap_or_default();
                let bf: Vec<String> = b.lines().filter(|l| l.starts_with("FILE ")).map(|l| l.split(" sha256=").next().unwrap_or(l).to_string()).collect();
                let af: Vec<String> = a.lines().filter(|l| l.starts_with("FILE ")).map(|l| l.split(" sha256=").next().unwrap_or(l).to_string()).collect();
                let mut diffs: Vec<String> = Vec::new();
                for f in &bf { if !af.contains(f) { diffs.push(format!("<= {f}")); } }
                for f in &af { if !bf.contains(f) { diffs.push(format!("=> {f}")); } }
                lines.push(format!("File metadata changes vs baseline: {}", diffs.len()));
                for d in diffs.iter().take(15) { lines.push(format!("  {d}")); }
            } else {
                lines.push("File metadata changes vs baseline: (no after snapshot)".to_string());
            }
            lines.push(String::new());
        }

        if count == 0 {
            lines.push("(no matches were recorded)".to_string());
            lines.push(String::new());
        }

        let mut all_endpoints: Vec<String> = net_rows.iter().filter_map(|r| {
            let addr = r.get(5)?.trim(); let port = r.get(6)?.trim();
            if addr.is_empty() || port.is_empty() { None } else { Some(format!("{addr}:{port}")) }
        }).collect();
        all_endpoints.sort(); all_endpoints.dedup();
        lines.push("=== ALL TCP ENDPOINTS OBSERVED ===".to_string());
        lines.push(if all_endpoints.is_empty() { "(none)".to_string() } else { all_endpoints.join(", ") });
        lines.push(String::new());
        lines.push("Reminder: endpoint and queue-time patterns are circumstantial. They do not reveal server-side skill ratings.".to_string());

        fs::write(&self.paths.report, lines.join("\n")).map_err(|e| e.to_string())?;
        self.log(&format!("Report written: {}", self.paths.report.display()));
        Ok(self.paths.report.clone())
    }

    pub fn zip_outputs(&self, zip_path: &Path) -> Result<(), String> {
        let mut files: Vec<PathBuf> = vec![
            self.paths.report.clone(),
            self.paths.tracker.clone(),
            self.paths.network_csv.clone(),
            self.paths.log.clone(),
        ];
        if let Ok(rd) = fs::read_dir(&self.paths.out_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file()
                    && let Some(name) = p.file_name().and_then(|s| s.to_str())
                    && name.starts_with("snapshot_") && name.ends_with(".txt")
                {
                    files.push(p);
                }
            }
        }
        files.retain(|p| p.exists());
        files.sort(); files.dedup();
        crate::zip_export::zip_files(&files, zip_path)
    }
}

fn parse_csv_simple(line: &str) -> Vec<String> {
    // Simple CSV (no quoted commas expected in our files)
    line.split(',').map(|s| s.to_string()).collect()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = std::io::Read::read(&mut f, &mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn queue_window_endpoints(net_rows: &[Vec<String>], qstart: &str, qend: &str) -> (usize, Vec<String>) {
    // qstart/qend are HH:MM:SS local; net rows have UTC ISO timestamps. Best effort: parse as today's local time.
    let today = chrono::Local::now().date_naive();
    let parse_hms = |s: &str| chrono::NaiveTime::parse_from_str(s.trim(), "%H:%M:%S").ok();
    let (Some(st), Some(et)) = (parse_hms(qstart), parse_hms(qend)) else { return (0, vec![]); };
    let start = today.and_time(st).and_local_timezone(chrono::Local).single().map(|d| d.with_timezone(&chrono::Utc));
    let end = today.and_time(et).and_local_timezone(chrono::Local).single().map(|d| d.with_timezone(&chrono::Utc));
    let (Some(start), Some(end)) = (start, end) else { return (0, vec![]); };
    let mut samples = 0usize;
    let mut eps: Vec<String> = Vec::new();
    for r in net_rows {
        let Some(ts) = r.first() else { continue; };
        let Ok(stamp) = chrono::DateTime::parse_from_rfc3339(ts) else { continue; };
        let stamp = stamp.with_timezone(&chrono::Utc);
        if stamp >= start && stamp <= end {
            samples += 1;
            let addr = r.get(5).map(|s| s.trim()).unwrap_or("");
            let port = r.get(6).map(|s| s.trim()).unwrap_or("");
            if !addr.is_empty() && !port.is_empty() {
                let ep = format!("{addr}:{port}");
                if !eps.contains(&ep) { eps.push(ep); }
            }
        }
    }
    eps.sort();
    (samples, eps)
}

#[cfg(target_os = "windows")]
fn collect_windows_process_conns() -> Result<Vec<String>, String> {
    // Read-only: query OS process/network metadata. No injection, no packet capture.
    let script = r#"$pattern='^(sp26-cod|cod|codcrashhandler|codbrokerservice|rico|bink)'; $procs=Get-Process | Where-Object { $_.ProcessName -match $pattern -and $_.ProcessName -notmatch 'opencode' }; if(-not $procs){ '(no COD/RICOCHET-related processes running)'; exit 0 }; foreach($p in $procs){ "PROC name=$($p.ProcessName) pid=$($p.Id) start=$($p.StartTime.ToString('o')) path=$($p.Path)"; $conns=Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object { $_.OwningProcess -eq $p.Id }; if($conns){ foreach($c in $conns){ "  CONN local=$($c.LocalAddress):$($c.LocalPort) remote=$($c.RemoteAddress):$($c.RemotePort) state=$($c.State)" } } else { '  CONN (none)' } }"#;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
        .output().map_err(|e| e.to_string())?;
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    if text.trim().is_empty() { text = String::from_utf8_lossy(&out.stderr).to_string(); }
    Ok(text.lines().map(|s| s.to_string()).collect())
}

#[cfg(target_os = "windows")]
fn collect_windows_tcp_csv_rows() -> Result<Vec<String>, String> {
    let script = r#"$pattern='^(sp26-cod|cod|codcrashhandler|codbrokerservice|rico|bink)'; $ts=(Get-Date).ToUniversalTime().ToString('o'); $procs=Get-Process | Where-Object { $_.ProcessName -match $pattern -and $_.ProcessName -notmatch 'opencode' }; foreach($p in $procs){ $conns=Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object { $_.OwningProcess -eq $p.Id }; foreach($c in $conns){ "$ts,$($p.ProcessName),$($p.Id),$($c.LocalAddress),$($c.LocalPort),$($c.RemoteAddress),$($c.RemotePort),$($c.State)" } }"#;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
        .output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.lines().filter(|l| !l.trim().is_empty()).map(|s| s.to_string()).collect())
}
