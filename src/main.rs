use dmz_sbmm_reader::app;
use dmz_sbmm_reader::scanner::{ScanOptions, default_keywords, detect_cod_paths, scan};
use rune::AppSpec;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn prefer_vulkan_backend() {
    if std::env::var_os("WGPU_BACKEND").is_none() {
        unsafe { std::env::set_var("WGPU_BACKEND", "Vulkan"); }
    }
}

#[derive(Debug, Default)]
struct Cli {
    scan_mode: bool,
    paths: Vec<PathBuf>,
    output: Option<PathBuf>,
    json_output: Option<PathBuf>,
    zip_output: Option<PathBuf>,
    max_file_mb: Option<u64>,
    no_size_limit: bool,
    skip_binary: bool,
    as_json: bool,
    help: bool,
}

fn is_flag(a: &str) -> bool {
    a.starts_with('-')
}

fn eq_flag(a: &str, names: &[&str]) -> bool {
    names.iter().any(|n| a.eq_ignore_ascii_case(n))
}

fn parse_cli() -> Result<Cli, String> {
    let mut cli = Cli::default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if eq_flag(a, &["--help", "-h", "/?"]) {
            cli.help = true;
        } else if eq_flag(a, &["--scan", "-Scan"]) {
            cli.scan_mode = true;
        } else if eq_flag(a, &["--as-json", "-AsJson"]) {
            cli.as_json = true;
            cli.scan_mode = true;
        } else if eq_flag(a, &["--no-size-limit", "-NoSizeLimit"]) {
            cli.no_size_limit = true;
            cli.scan_mode = true;
        } else if eq_flag(a, &["--skip-binary", "-SkipBinary"]) {
            cli.skip_binary = true;
            cli.scan_mode = true;
        } else if eq_flag(a, &["--output", "-Output"]) {
            i += 1;
            cli.output = Some(PathBuf::from(args.get(i).ok_or("Missing value for --output")?));
            cli.scan_mode = true;
        } else if eq_flag(a, &["--json-output", "-JsonOutput"]) {
            i += 1;
            cli.json_output = Some(PathBuf::from(args.get(i).ok_or("Missing value for --json-output")?));
            cli.scan_mode = true;
        } else if eq_flag(a, &["--zip-output", "-ZipOutput"]) {
            i += 1;
            cli.zip_output = Some(PathBuf::from(args.get(i).ok_or("Missing value for --zip-output")?));
            cli.scan_mode = true;
        } else if eq_flag(a, &["--max-file-mb", "-MaxFileMB"]) {
            i += 1;
            let v = args.get(i).ok_or("Missing value for --max-file-mb")?;
            cli.max_file_mb = Some(v.parse::<u64>().map_err(|_| format!("Invalid --max-file-mb: {v}"))?);
            cli.scan_mode = true;
        } else if eq_flag(a, &["--paths", "-Paths"]) {
            cli.scan_mode = true;
            i += 1;
            while i < args.len() && !is_flag(&args[i]) {
                cli.paths.push(PathBuf::from(&args[i]));
                i += 1;
            }
            continue;
        } else if eq_flag(a, &["--pass-thru", "-PassThru"]) {
            // For CLI compatibility; JSON output is the AI-friendly form.
            cli.as_json = true;
            cli.scan_mode = true;
        } else if is_flag(a) {
            return Err(format!("Unknown argument: {a}"));
        } else {
            // Positional path convenience
            cli.paths.push(PathBuf::from(a));
            cli.scan_mode = true;
        }
        i += 1;
    }
    Ok(cli)
}

fn print_help() {
    println!("DMZ SBMM Reader (Rust/Rune) - read-only local scanner");
    println!();
    println!("GUI (default): dmz-sbmm-reader");
    println!("CLI scan:      dmz-sbmm-reader --scan [options]");
    println!();
    println!("Options:");
    println!("  --paths <dir> [dir...]   Folders to scan (auto-detected if omitted)");
    println!("  --output <file>          Text report path");
    println!("  --json-output <file>     JSON report path");
    println!("  --zip-output <file>      ZIP containing report files");
    println!("  --max-file-mb <n>        Max file size in MB (default 32)");
    println!("  --no-size-limit          Scan all file sizes");
    println!("  --skip-binary            Skip ASCII extraction from non-text files");
    println!("  --as-json                Print JSON result to stdout (AI-friendly)");
    println!("  --help                   Show this help");
}

fn run_scan_cli(cli: Cli) -> Result<(), String> {
    let paths = if cli.paths.is_empty() { detect_cod_paths() } else { cli.paths };
    let out_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let opts = ScanOptions {
        paths,
        output: cli.output.unwrap_or_else(|| out_dir.join("DMZ_SBMM_Reader_result.txt")),
        json_output: cli.json_output,
        zip_output: cli.zip_output,
        max_file_mb: cli.max_file_mb.unwrap_or(32),
        no_size_limit: cli.no_size_limit,
        skip_binary: cli.skip_binary,
        keywords: default_keywords(),
    };
    let result = scan(&opts)?;
    if cli.as_json {
        let json = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
        println!("{json}");
    } else {
        println!("WROTE: {}", result.output);
        if let Some(j) = &result.json_output { println!("JSON: {j}"); }
        if let Some(z) = &result.zip_output { println!("ZIP: {z}"); }
        println!(
            "Scanned: {} | Skipped: {} | Errors: {} | Hits: {}",
            result.summary.scanned, result.summary.skipped, result.summary.errors, result.summary.hits
        );
    }
    Ok(())
}

fn main() {
    #[cfg(target_os = "windows")]
    prefer_vulkan_backend();

    let cli = match parse_cli() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if cli.help {
        print_help();
        return;
    }
    if cli.scan_mode {
        if let Err(e) = run_scan_cli(cli) {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
        return;
    }

    let spec = AppSpec {
        min_size: Some((760.0, 600.0)),
        ..AppSpec::new(
            "DMZ SBMM Reader",
            (980.0, 720.0),
            app::State::default,
            app::view,
        )
    };

    rune::App::new(spec)
        .titlebar(rune::Titlebar::new(40.0))
        .run();
}
