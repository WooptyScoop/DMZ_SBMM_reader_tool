use dmz_sbmm_reader::scanner::{scan, ScanOptions};
use std::fs;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("dmz-rune-test-{name}-{}", std::process::id()))
}

#[test]
fn scan_finds_keywords_and_writes_outputs() {
    let root = temp("scan");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "SBMM matchmaking test").unwrap();
    fs::write(root.join("b.bin"), b"\0\0mmr skill rating\0").unwrap();

    let out = root.join("result.txt");
    let json = root.join("result.json");
    let zip = root.join("result.zip");
    let opts = ScanOptions {
        paths: vec![root.clone()],
        output: out.clone(),
        json_output: Some(json.clone()),
        zip_output: Some(zip.clone()),
        max_file_mb: 32,
        no_size_limit: false,
        skip_binary: false,
        keywords: vec!["sbmm".into(), "mmr".into(), "skill rating".into()],
    };
    let r = scan(&opts).unwrap();
    assert!(r.summary.hits >= 2);
    assert!(out.exists());
    assert!(json.exists());
    assert!(zip.exists());
    let txt = fs::read_to_string(&out).unwrap();
    assert!(txt.contains("=== SUMMARY ==="));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn zip_export_creates_archive() {
    let root = temp("zip");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "a").unwrap();
    fs::write(&b, "b").unwrap();
    let z = root.join("out.zip");
    dmz_sbmm_reader::zip_export::zip_files(&[a, b], &z).unwrap();
    assert!(z.exists());
    let _ = fs::remove_dir_all(&root);
}
