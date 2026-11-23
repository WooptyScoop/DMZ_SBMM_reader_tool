use dmz_sbmm_reader::probe::Probe;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dmz-full-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn cli_scan_all_options() {
    let dir = temp_dir("cli");
    let input = dir.join("input");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.txt"), "sbmm mmr matchmaking skill rating").unwrap();
    fs::write(input.join("b.bin"), b"\0\0sbmm\0mmr\0").unwrap();

    let out = dir.join("scan.txt");
    let json = dir.join("scan.json");
    let zip = dir.join("scan.zip");
    let exe = env!("CARGO_BIN_EXE_dmz-sbmm-reader");
    let status = Command::new(exe)
        .args([
            "--scan",
            "--paths", input.to_str().unwrap(),
            "--output", out.to_str().unwrap(),
            "--json-output", json.to_str().unwrap(),
            "--zip-output", zip.to_str().unwrap(),
            "--no-size-limit", "--as-json",
        ])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(out.exists(), "text report missing");
    assert!(json.exists(), "json report missing");
    assert!(zip.exists(), "zip report missing");
    let j: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(j["summary"]["hits"].as_u64().unwrap() >= 4, "hits: {j}");
    assert_eq!(j["summary"]["scanned"].as_u64().unwrap(), 2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cli_help_and_invalid_arg() {
    let exe = env!("CARGO_BIN_EXE_dmz-sbmm-reader");
    let help = Command::new(exe).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--scan"));

    let bad = Command::new(exe).arg("--definitely-not-a-flag").output().unwrap();
    assert!(!bad.status.success(), "unknown flag should fail");
}

#[test]
fn probe_full_workflow() {
    let dir = temp_dir("probe");
    let input = dir.join("scans");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("cod26_config.txt"), "skill_based_matchmaking=1").unwrap();
    let out = dir.join("probeout");

    let mut p = Probe::new(out.clone(), vec![input.clone()]);
    p.init_files().unwrap();
    assert!(p.paths.tracker.exists(), "tracker missing");
    assert!(p.paths.network_csv.exists(), "network csv missing");

    p.mark_queue_start().unwrap();
    assert!(p.baseline_done, "baseline flag not set");
    assert!(p.queue_start.is_some(), "queue start not recorded");
    assert!(out.join("snapshot_baseline.txt").exists(), "baseline snapshot missing");
    assert!(out.join("snapshot_m01_queue.txt").exists(), "queue snapshot missing");

    thread::sleep(Duration::from_millis(1100));
    let sec = p.mark_queue_end().unwrap();
    assert!(sec >= 1.0, "queue seconds too short: {sec}");
    assert!(out.join("snapshot_m01_queue_end.txt").exists());
    let tracker = fs::read_to_string(&p.paths.tracker).unwrap();
    assert!(tracker.contains("1,"), "tracker row missing: {tracker}");
    assert_eq!(tracker.lines().count(), 2, "tracker should have header + 1 row");

    p.mark_match_end().unwrap();
    assert_eq!(p.match_number, 2, "match number did not advance");
    assert!(out.join("snapshot_m01_after.txt").exists());

    p.sample_network().unwrap(); // must be Ok even with no COD processes running

    let report = p.write_report().unwrap();
    assert!(report.exists(), "report missing");
    let rep = fs::read_to_string(&report).unwrap();
    assert!(rep.contains("MATCH 1"), "report missing match: {rep}");
    assert!(rep.contains("Queue TCP samples"), "report missing TCP section");
    assert!(rep.contains("=== ALL TCP ENDPOINTS OBSERVED ==="));

    let zp = dir.join("probe.zip");
    p.zip_outputs(&zp).unwrap();
    assert!(zp.exists(), "probe zip missing");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn probe_queue_end_without_start_errors() {
    let dir = temp_dir("probe_err");
    let mut p = Probe::new(dir.clone(), vec![dir.clone()]);
    p.init_files().unwrap();
    assert!(p.mark_queue_end().is_err(), "queue end without start should error");
    let _ = fs::remove_dir_all(&dir);
}
