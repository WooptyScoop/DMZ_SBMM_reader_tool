use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

pub fn zip_files(files: &[PathBuf], zip_path: &Path) -> Result<(), String> {
    if files.is_empty() {
        return Err("No files to zip".to_string());
    }
    if let Some(parent) = zip_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if zip_path.exists() {
        std::fs::remove_file(zip_path).map_err(|e| e.to_string())?;
    }
    let file = File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut added = 0usize;
    for path in files {
        if !path.exists() || !path.is_file() { continue; }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file").to_string();
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        let mut f = File::open(path).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        zip.write_all(&buf).map_err(|e| e.to_string())?;
        added += 1;
    }
    zip.finish().map_err(|e| e.to_string())?;
    if added == 0 {
        let _ = std::fs::remove_file(zip_path);
        return Err("No existing files to zip".to_string());
    }
    Ok(())
}
