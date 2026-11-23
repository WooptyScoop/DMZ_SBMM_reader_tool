use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScanSummary {
    pub scanned: usize,
    pub skipped: usize,
    pub errors: usize,
    pub hits: usize,
    pub hit_files: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HitFile {
    pub path: String,
    pub size: u64,
    pub modified_utc: String,
    pub hits: usize,
    pub samples: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScanResult {
    pub started: String,
    pub finished: String,
    pub paths: Vec<String>,
    pub output: String,
    pub json_output: Option<String>,
    pub zip_output: Option<String>,
    pub no_size_limit: bool,
    pub max_file_mb: Option<u64>,
    pub skip_binary: bool,
    pub keywords: Vec<String>,
    pub summary: ScanSummary,
    pub hit_files: Vec<HitFile>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProbeRow {
    pub match_num: i32,
    pub queue_start: String,
    pub queue_end: String,
    pub queue_seconds: f64,
    pub prev_kills: String,
    pub prev_deaths: String,
    pub prev_exfil: String,
    pub lobby_avg_level: String,
    pub lobby_avg_kd: String,
    pub notes: String,
}

#[derive(Clone, Debug)]
pub struct ProbePaths {
    pub out_dir: PathBuf,
    pub tracker: PathBuf,
    pub network_csv: PathBuf,
    pub report: PathBuf,
    pub log: PathBuf,
}

impl ProbePaths {
    pub fn new(out_dir: PathBuf) -> Self {
        Self {
            tracker: out_dir.join("dmz_sbmm_tracker.csv"),
            network_csv: out_dir.join("network_watch.csv"),
            report: out_dir.join("DMZ_SBMM_Probe_Report.txt"),
            log: out_dir.join("probe_status.txt"),
            out_dir,
        }
    }
}

pub fn now_utc_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn now_local_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
