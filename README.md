# DMZ SBMM Reader (Rust/Rune rewrite)

A **read-only** local Call of Duty / DMZ metadata toolkit, rewritten in **Rust** with the **Rune** native GPU-rendered UI framework.

> **Read-only by design.** No injection, no hooking, no memory reads, no process modification, no anti-cheat bypass, no network interception, and no writes into game folders.

> **Open source and free to modify.** This tool is released under the MIT License. Anyone can read, use, modify, fork, and redistribute the source code - including changing how it works - as long as the MIT license notice is kept. See [License](#license).

## Stack

| Layer | Choice |
|---|---|
| Main application | Rust (Edition 2024) |
| UI framework | [Rune](https://github.com/anomalyco/rune) |
| Layout | Taffy (used by Rune) |
| Rendering | GPU accelerated (Rune / wgpu) |
| Windows graphics | Vulkan requested (`WGPU_BACKEND=Vulkan` is set at startup when unset; the wgpu Vulkan feature is enabled) |

> **Graphics note:** Rune's upstream Windows default is DX12. This rewrite *requests* Vulkan at startup and enables the wgpu Vulkan feature, but does not strictly guarantee it. If Vulkan is unavailable, wgpu may fall back to another adapter according to its normal selection rules.

## What the tool does

There are two main functions, available as tabs in the GUI. Both are strictly read-only.

### 1. File scan

Searches local Call of Duty files for SBMM / matchmaking-related strings.

- **Keywords (case-insensitive):** `sbmm`, `skill based`, `skill-based`, `skill_based`, `matchmaking`, `mmr`, `elo`, `skill rating`, `skill bracket`, `meld`, `dmz`. Short terms (4 characters or fewer) are matched with word boundaries to reduce false positives.
- **Path discovery:** if you don't supply folders, the tool auto-detects common locations - `Documents\Call of Duty`, `LOCALAPPDATA\Activision\Call of Duty`, `LOCALAPPDATA\Call of Duty`, `ProgramData\Call of Duty`, Steam libraries, and XboxGames / Call of Duty folders across drives A:–Z:. The "Detect paths" button fills the path box with what it found; you can also type your own folders, one per line.
- **File handling:** walks folders without following symlinks. A **32 MB** per-file cap applies by default; "No size limit" removes it. Text files (`.txt`, `.json`, `.xml`, `.cfg`, `.ini`, `.log`, `.csv`) are read as UTF-8. Other files are scanned by extracting printable ASCII runs of 5+ characters, unless "Skip binary" is enabled. Up to 8 context samples per matched file are recorded.
- **Outputs:** a text report, an optional JSON report, and an optional ZIP containing both. The report lists matched files with path, size, modification time, hit count, and samples, plus a summary (files scanned, files skipped, read errors, total hits).
- **Important limitation:** a lack of matches does **not** prove SBMM is absent. Matchmaking decisions are made server-side and are normally not stored in client files.

### 2. Match probe

Records read-only process, network, and file-metadata observations around queue/match events.

- **Workflow buttons:** `1. Queue started` → `2. Match found` → `3. Match ended`, then `Stop & report`. You can record multiple matches in one session.
- **Snapshots:** the first queue start takes a full baseline snapshot. Each match records a light queue snapshot, a light queue-end snapshot, and a full "after" snapshot. Light snapshots keep only COD-related filenames; full snapshots record file metadata for files under 10 MB and SHA-256 hashes for files under 1 MB.
- **5-second network watch:** a background thread samples Windows TCP connection metadata (via `Get-Process` / `Get-NetTCPConnection`) for COD/RICOCHET-related processes every 5 seconds into `network_watch.csv`. It starts automatically with the app, is shown as `Net watch: ON/OFF` in the status, and stops when you press `Stop & report`. This is read-only OS metadata - **no packet capture and no interception**.
- **Report:** `Stop & report` stops the sampler and writes a report that correlates each match's queue window with sampled TCP endpoints, compares file metadata against the baseline, and lists all observed endpoints. The report explicitly notes that endpoint and queue-time patterns are circumstantial and do not reveal server-side skill ratings.
- **Export ZIP:** packages the report, tracker CSV, network CSV, status log, and all snapshots.

### 3. Export ZIP

Both functions have an "Export ZIP..." button that opens a save dialog and packages the relevant reports into a ZIP of your choosing. Nothing is written into game folders.

### 4. CLI / AI-friendly mode

The file scanner can run without the GUI and emit structured JSON for scripts and AI agents. See [Run](#run) below.

## Safety / guarantees

- Does **not** inject into the game.
- Does **not** hook game functions.
- Does **not** read game memory.
- Does **not** modify processes or game files.
- Does **not** bypass anti-cheat.
- Does **not** intercept or decrypt TLS, and does **not** capture packets.
- Only writes report/export files you explicitly request, outside game folders (by default next to the executable, or wherever you choose in a save dialog).

## Build

Requires a Rust toolchain and, on Windows, Visual Studio C++ build tools (MSVC). A Vulkan-capable GPU/driver is needed for the Vulkan backend.

```powershell
cargo build --release
```

> **Self-contained build:** Rune is vendored in `vendor/rune` and referenced with relative paths in `Cargo.toml`. No external checkout or `%TEMP%` folder is needed - just extract this package and run `cargo build --release`. On Windows, `.cargo/config.toml` links the C runtime statically so the resulting `dmz-sbmm-reader.exe` runs on a normal Windows 10/11 install without the Visual C++ Redistributable.

## Run

### GUI

If you downloaded a release ZIP, extract it and double-click `RUN DMZ SBMM Reader.cmd` (or `dmz-sbmm-reader.exe` directly). Reports are written next to the executable.

If you built from source:

```powershell
.\target\release\dmz-sbmm-reader.exe
```

The GUI opens in a dark theme with a left sidebar and a restrained lime accent. The two views are **File scan** and **Match probe**. File scans run synchronously, so the window may briefly stop repainting while a large scan is in progress.

### CLI / AI use

Use `--as-json` to print a machine-readable JSON result to stdout.

```powershell
# Human-readable scan
.\target\release\dmz-sbmm-reader.exe --scan --paths "D:\XboxGames\Call of Duty" --output .\scan.txt

# AI / automation-friendly JSON on stdout
.\target\release\dmz-sbmm-reader.exe --scan --paths "D:\XboxGames\Call of Duty" --json-output .\scan.json --as-json

# Include a ZIP of the reports
.\target\release\dmz-sbmm-reader.exe --scan --paths "D:\XboxGames\Call of Duty" --output .\scan.txt --json-output .\scan.json --zip-output .\scan.zip --as-json
```

Supported CLI options:

- `--scan` - run the scanner in CLI mode (no GUI)
- `--paths <dir> [dir...]` - folders to scan; auto-detected if omitted
- `--output <file>` - text report path
- `--json-output <file>` - JSON report path
- `--zip-output <file>` - ZIP containing the generated reports
- `--max-file-mb <n>` - maximum file size in MB (default 32)
- `--no-size-limit` - scan files of all sizes
- `--skip-binary` - skip ASCII extraction from non-text files
- `--as-json` - print the JSON result to stdout
- `--pass-thru` - alias for `--as-json`
- `--help` - show CLI help

## Tests

```powershell
cargo test --release
```

The test suite covers CLI scanning, JSON output, ZIP export, the full probe workflow, error handling, background sampler start/stop, and an end-to-end check that the sampler captures TCP connection data from a simulated process.

## Project layout

- `src/main.rs` - application entry, Vulkan preference, GUI launch, and CLI scan mode.
- `src/app.rs` - Rune UI, state, sampler thread, and button handlers.
- `src/scanner.rs` - read-only file scanner and path detection.
- `src/probe.rs` - read-only match probe, snapshots, network sampling, and report generation.
- `src/zip_export.rs` - ZIP export helper.
- `src/model.rs` - shared data types and timestamps.
- `tests/` - integration tests for both functions and the sampler.

## License

MIT. See `LICENSE` and `NOTICE`.

This project is **open source**: anyone may view, use, copy, modify, fork, and redistribute the source code, including making their own changes to how the tool works. The only requirement of the MIT License is that the copyright notice and license text are included in copies or substantial portions of the software. The software is provided "as is", without warranty.

Third-party dependencies keep their own licenses. The vendored Rune framework is MIT-licensed (`vendor/rune/LICENSE`), with additional dependency notices in `vendor/rune/THIRD_PARTY_NOTICES.md`.

## Credits

**Made by WooptyScoop with the aid of Big Pickle and Willsman.**

- **WooptyScoop** - scope, safety rules, read-only design, feature choices, and final release decisions.
- **Big Pickle** (AI assistant) - drafting, implementation, review, and documentation.
- **Willsman** - improved UI.
