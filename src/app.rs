use crate::model::now_local_hms;
use crate::probe::Probe;
use crate::scanner::{ScanOptions, default_keywords, detect_cod_paths, scan};
use crate::ui::{self, Palette, icons};
use rune::tok::{space, type_};
use rune::{Cx, El, Icon, ScrollSpec, TextEdit, col, el, icon, row, text, text_input};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Scan,
    Probe,
}

pub struct State {
    pub tab: Tab,
    // scan
    pub paths_edit: TextEdit,
    pub max_mb: u64,
    pub no_size_limit: bool,
    pub skip_binary: bool,
    pub scan_status: String,
    pub scan_log: Vec<String>,
    pub scan_report: Option<PathBuf>,
    pub scan_json: Option<PathBuf>,
    pub scanning: bool,
    pub scan_result: Arc<Mutex<Option<Result<crate::model::ScanResult, String>>>>,
    // probe
    pub probe: Option<Arc<Mutex<Probe>>>,
    pub probe_status: String,
    pub probe_log: Vec<String>,
    pub probe_done: bool,
    // background read-only network sampler (5s interval)
    pub net_sampler_stop: Arc<AtomicBool>,
    pub net_sampler_thread: Option<JoinHandle<()>>,
}

impl Default for State {
    fn default() -> Self {
        let paths = detect_cod_paths();
        let paths_text = if paths.is_empty() {
            String::new()
        } else {
            paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n")
        };
        let mut s = Self {
            tab: Tab::Scan,
            paths_edit: TextEdit::new(&paths_text),
            max_mb: 32,
            no_size_limit: false,
            skip_binary: false,
            scan_status: "Ready.".to_string(),
            scan_log: vec!["DMZ SBMM Reader (Rust/Rune) started. Read-only.".to_string()],
            scan_report: None,
            scan_json: None,
            scanning: false,
            scan_result: Arc::new(Mutex::new(None)),
            probe: None,
            probe_status: "Match 1   |   Queue: not started".to_string(),
            probe_log: vec!["Match probe ready. Read-only metadata only.".to_string()],
            probe_done: false,
            net_sampler_stop: Arc::new(AtomicBool::new(false)),
            net_sampler_thread: None,
        };
        s.ensure_probe();
        s.update_probe_status();
        s
    }
}

impl State {
    fn out_dir() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    fn ensure_probe(&mut self) {
        if self.probe.is_some() { return; }
        let out = Self::out_dir();
        let scan_paths = detect_cod_paths();
        let probe = Probe::new(out, scan_paths);
        let _ = probe.init_files();
        probe.log("DMZ SBMM Match Probe started (read-only).");
        self.probe = Some(Arc::new(Mutex::new(probe)));
        self.start_net_sampler();
    }

    fn probe_mut<R>(&self, f: impl FnOnce(&mut Probe) -> R) -> Result<R, String> {
        let guard = self.probe.as_ref().ok_or("Probe not initialized")?;
        let mut p = guard.lock().map_err(|_| "Probe lock poisoned".to_string())?;
        Ok(f(&mut p))
    }

    fn probe_ref<R>(&self, f: impl FnOnce(&Probe) -> R) -> Result<R, String> {
        let guard = self.probe.as_ref().ok_or("Probe not initialized")?;
        let p = guard.lock().map_err(|_| "Probe lock poisoned".to_string())?;
        Ok(f(&p))
    }

    fn start_net_sampler(&mut self) {
        if self.net_sampler_thread.is_some() { return; }
        let Some(probe) = self.probe.clone() else { return; };
        self.net_sampler_stop.store(false, Ordering::Relaxed);
        let stop = self.net_sampler_stop.clone();
        self.net_sampler_thread = Some(thread::spawn(move || loop {
            if stop.load(Ordering::Relaxed) { break; }
            match probe.lock() {
                Ok(p) => {
                    if let Err(e) = p.sample_network() {
                        p.log(&format!("Network sample error: {e}"));
                    }
                }
                Err(_) => break,
            }
            for _ in 0..50 {
                if stop.load(Ordering::Relaxed) { return; }
                thread::sleep(Duration::from_millis(100));
            }
        }));
        self.push_probe_log("Network watch started (5s interval, read-only).");
        let _ = self.probe_ref(|p| p.log("Network watch started (5s interval, read-only)."));
    }

    fn stop_net_sampler(&mut self) {
        if self.net_sampler_thread.is_none() { return; }
        self.net_sampler_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.net_sampler_thread.take() {
            let _ = handle.join();
        }
        self.push_probe_log("Network watch stopped.");
        let _ = self.probe_ref(|p| p.log("Network watch stopped."));
    }

    fn push_scan_log(&mut self, msg: impl Into<String>) {
        let line = format!("{}  {}", now_local_hms(), msg.into());
        self.scan_log.push(line);
        if self.scan_log.len() > 500 { let drain = self.scan_log.len() - 500; self.scan_log.drain(0..drain); }
    }
    fn push_probe_log(&mut self, msg: impl Into<String>) {
        let line = format!("{}  {}", now_local_hms(), msg.into());
        self.probe_log.push(line);
        if self.probe_log.len() > 500 { let drain = self.probe_log.len() - 500; self.probe_log.drain(0..drain); }
    }

    fn update_probe_status(&mut self) {
        let info = self.probe_ref(|p| (p.match_number, p.queue_start.is_some()));
        if let Ok((n, started)) = info {
            let q = if started { "started" } else { "not started" };
            self.probe_status = format!("Match {n}   |   Queue: {q}");
        }
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = ui::dark();
    let ci = cx.chrome_insets();
    let header_h = ci.height.max(44.0);

    col().key("root").w_full().h_full().fill(p.bg)
        .child(titlebar(&p, ci.leading, ci.trailing, header_h))
        .child(
            row().key("body").w_full().grow(1.0).min_h(0.0)
                .child(sidebar(s, &p))
                .child(
                    el().key("content-wrap").grow(1.0).min_w(0.0).px(space::S6).py(space::S3)
                        .child(match s.tab {
                            Tab::Scan => scan_view(s, &p),
                            Tab::Probe => probe_view(s, &p),
                        }),
                ),
        )
}

fn titlebar(p: &Palette, leading: f32, trailing: f32, h: f32) -> El {
    row().key("titlebar").w_full().h(h)
        .pl(space::S6 + leading).pr(space::S6 + trailing)
        .center_y().justify_between().window_drag()
        .fill(p.sidebar)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.border)
        .child(
            row().key("brand").gap(10.0).center_y()
                .child(el().key("brand-mark").size(12.0, 12.0).radius(3.0).fill(p.accent))
                .child(text("DMZ SBMM Reader").font(type_::TITLE).color(p.text)),
        )
        .child(
            row().key("brand-meta").gap(10.0).center_y()
                .child(ui::dot(p.accent, 5.0))
                .child(text("READ-ONLY").font(ui::CAPTION.tracking(0.8)).color(p.secondary))
                .child(text("v0.1.0").font(ui::CAPTION).color(p.muted)),
        )
}

fn sidebar(s: &State, p: &Palette) -> El {
    col().key("sidebar").w(216.0).fill(p.sidebar)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.border)
        .px(12.0).pt(16.0).pb(14.0)
        .child(ui::eyebrow("TOOLS", p).pl(10.0))
        .child(el().key("nav-gap-top").h(10.0))
        .child(
            col().key("nav").gap(2.0)
                .child(nav_item("nav-scan", "File scan", &icons::SCAN, s.tab == Tab::Scan, p, |st: &mut State| st.tab = Tab::Scan))
                .child(nav_item("nav-probe", "Match probe", &icons::PROBE, s.tab == Tab::Probe, p, |st: &mut State| st.tab = Tab::Probe)),
        )
        .child(el().key("nav-spacer").grow(1.0))
        .child(ui::hairline(p))
        .child(
            row().key("sidebar-note").gap(9.0).center_y().pt(12.0)
                .child(icon(&icons::SHIELD, 15.0).color(p.accent))
                .child(
                    col().key("sidebar-note-text").gap(2.0)
                        .child(text("Read-only by design").font(type_::LABEL).color(p.secondary))
                        .child(text("Nothing leaves this machine.").font(ui::CAPTION).color(p.muted)),
                ),
        )
}

fn nav_item(
    key: &'static str,
    label: &'static str,
    glyph: &'static Icon,
    active: bool,
    p: &Palette,
    on: fn(&mut State),
) -> El {
    row().key(key).h(34.0).px(10.0).gap(10.0).center_y().radius(6.0)
        .fill(if active { p.selected } else { p.transparent })
        .hover(p.hover)
        .on_press(on)
        .child(icon(glyph, 15.0).color(if active { p.accent } else { p.secondary }))
        .child(text(label).font(type_::LABEL).color(if active { p.text } else { p.secondary }))
        .child(el().key("nav-fill").grow(1.0))
        .child(if active { ui::dot(p.accent, 5.0) } else { el().key("nav-empty") })
}

fn scan_view(s: &State, p: &Palette) -> El {
    let log_text = if s.scan_log.is_empty() {
        "No log.".to_string()
    } else {
        s.scan_log.iter().rev().take(80).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
    };

    col().key("scan").w_full().h_full().gap(space::S6)
        .child(ui::page_header("File scan", "Search local Call of Duty files for SBMM-related strings.", p))
        .child(
            ui::card("folders-card", p).px(16.0).py(14.0)
                .child(
                    col().key("folders").w_full().gap(9.0)
                        .child(row().key("folders-head").w_full().justify_between().center_y()
                            .child(ui::eyebrow("FOLDERS TO SCAN", p))
                            .child(text("one per line").font(ui::CAPTION).color(p.muted)))
                        .child(
                            el().key("paths-field").w_full().h(84.0)
                                .radius(7.0).fill(p.well).border(1.0, p.border).px(10.0).py(8.0)
                                .child(text_input(&s.paths_edit, type_::BODY).key("paths-input").w_full().color(p.text)
                                    .on_edit(|st: &mut State| &mut st.paths_edit)),
                        )
                        .child(ui::hint("Leave blank to use auto-detected folders, or press Detect paths.", p)),
                ),
        )
        .child(
            row().key("scan-actions").gap(space::S2).center_y()
                .child(secondary_btn("scan-detect", "Detect paths", &icons::DETECT, p, detect_paths))
                .child(primary_btn("scan-now", "Scan now", &icons::PLAY, p, do_scan))
                .child(toggle_btn("skip-bin", "Skip binary", s.skip_binary, p, |st| st.skip_binary = !st.skip_binary))
                .child(toggle_btn("no-limit", "No size limit", s.no_size_limit, p, |st| st.no_size_limit = !st.no_size_limit))
                .child(el().key("actions-fill").grow(1.0))
                .child(secondary_btn("scan-zip", "Export ZIP", &icons::EXPORT, p, export_scan_zip)),
        )
        .child(
            row().key("scan-status").gap(9.0).center_y()
                .child(ui::dot(if s.scanning { p.accent } else { p.green }, 6.0))
                .child(text(s.scan_status.clone()).font(type_::BODY).color(p.secondary)),
        )
        .child(
            ui::card("scan-log-card", p).grow(1.0).min_h(140.0).px(14.0).py(12.0)
                .child(
                    col().key("scan-log").w_full().h_full().gap(9.0)
                        .child(row().key("scan-log-head").gap(8.0).center_y()
                            .child(icon(&icons::LIST, 14.0).color(p.muted))
                            .child(ui::eyebrow("LOG / RESULTS", p)))
                        .child(
                            ui::well("scan-log-box", p).grow(1.0).min_h(0.0).px(12.0).py(10.0)
                                .scroll(ScrollSpec { stick_bottom: false })
                                .child(text(log_text).font(ui::MONO).color(p.secondary)),
                        ),
                ),
        )
        .child(ui::hairline(p))
        .child(footer_note("Read-only. Nothing is written into game folders. No matches does not prove SBMM is absent.", p))
}

fn probe_view(s: &State, p: &Palette) -> El {
    let watch = if s.net_sampler_thread.is_some() { "ON" } else { "OFF" };
    let log_text = if s.probe_log.is_empty() {
        "No log.".to_string()
    } else {
        s.probe_log.iter().rev().take(120).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
    };

    col().key("probe").w_full().h_full().gap(space::S6)
        .child(ui::page_header("Match probe", "Record read-only process, network and file metadata around a match.", p))
        .child(
            ui::card("probe-status-card", p).px(16.0).py(14.0)
                .child(row().key("probe-status-row").w_full().center_y().justify_between()
                    .child(col().key("probe-status-text").gap(5.0)
                        .child(ui::eyebrow("CURRENT MATCH", p))
                        .child(text(s.probe_status.clone()).font(type_::TITLE).color(p.text)))
                    .child(row().key("watch-pill").gap(8.0).center_y().px(10.0).py(6.0)
                        .radius(6.0).fill(p.well).border(1.0, p.border)
                        .child(ui::dot(if watch == "ON" { p.green } else { p.muted }, 6.0))
                        .child(text(format!("Net watch: {watch}")).font(type_::LABEL).color(p.secondary)))),
        )
        .child(
            ui::card("workflow-card", p).px(16.0).py(14.0)
                .child(col().key("workflow").w_full().gap(12.0)
                    .child(row().key("workflow-head").w_full().justify_between().center_y()
                        .child(ui::eyebrow("WORKFLOW", p))
                        .child(text("steps run in order").font(ui::CAPTION).color(p.muted)))
                    .child(row().key("workflow-steps").w_full().gap(space::S2).center_y()
                        .child(step_btn("qstart", "1", "Queue started", p, probe_queue_start))
                        .child(step_btn("qend", "2", "Match found", p, probe_queue_end))
                        .child(step_btn("mend", "3", "Match ended", p, probe_match_end)))
                    .child(ui::hairline(p))
                    .child(row().key("workflow-actions").w_full().gap(space::S2).center_y()
                        .child(primary_btn("pstop", "Stop & report", &icons::STOP, p, probe_stop))
                        .child(secondary_btn("pzip", "Export ZIP", &icons::EXPORT, p, export_probe_zip))
                        .child(el().key("workflow-fill").grow(1.0))
                        .child(text("writes a report when you stop").font(ui::CAPTION).color(p.muted)))),
        )
        .child(
            ui::card("probe-log-card", p).grow(1.0).min_h(140.0).px(14.0).py(12.0)
                .child(col().key("probe-log").w_full().h_full().gap(9.0)
                    .child(row().key("probe-log-head").gap(8.0).center_y()
                        .child(icon(&icons::LIST, 14.0).color(p.muted))
                        .child(ui::eyebrow("ACTIVITY LOG", p)))
                    .child(ui::well("probe-log-box", p).grow(1.0).min_h(0.0).px(12.0).py(10.0)
                        .scroll(ScrollSpec { stick_bottom: false })
                        .child(text(log_text).font(ui::MONO).color(p.secondary)))),
        )
        .child(ui::hairline(p))
        .child(footer_note("No injection, no memory reads, no network interception, no writes into game folders.", p))
}

fn footer_note(msg: &str, p: &Palette) -> El {
    row().key("footer-note").gap(8.0).center_y()
        .child(icon(&icons::SHIELD, 13.0).color(p.muted))
        .child(text(msg.to_string()).font(ui::CAPTION).color(p.muted))
}

fn primary_btn(key: &str, label: &str, glyph: &'static Icon, p: &Palette, on: fn(&mut State)) -> El {
    row().key(key.to_string()).h(32.0).px(13.0).gap(8.0).center_y().radius(6.0)
        .fill(p.accent).hover(p.accent).press(p.accent)
        .on_press(on)
        .child(icon(glyph, 14.0).color(p.accent_text))
        .child(text(label.to_string()).font(type_::LABEL).color(p.accent_text))
}

fn secondary_btn(key: &str, label: &str, glyph: &'static Icon, p: &Palette, on: fn(&mut State)) -> El {
    row().key(key.to_string()).h(32.0).px(13.0).gap(8.0).center_y().radius(6.0)
        .fill(p.card).border(1.0, p.border_strong).hover(p.hover)
        .on_press(on)
        .child(icon(glyph, 14.0).color(p.secondary))
        .child(text(label.to_string()).font(type_::LABEL).color(p.text))
}

fn toggle_btn(key: &str, label: &str, on: bool, p: &Palette, f: fn(&mut State)) -> El {
    row().key(key.to_string()).h(32.0).px(11.0).gap(8.0).center_y().radius(6.0)
        .fill(p.card).border(1.0, if on { p.accent_soft } else { p.border }).hover(p.hover)
        .on_press(f)
        .child(ui::dot(if on { p.accent } else { p.muted }, 6.0))
        .child(text(label.to_string()).font(type_::LABEL).color(if on { p.text } else { p.secondary }))
}

fn step_btn(key: &str, num: &str, label: &str, p: &Palette, on: fn(&mut State)) -> El {
    row().key(key.to_string()).h(36.0).px(12.0).gap(9.0).center_y().radius(6.0)
        .fill(p.card).border(1.0, p.border_strong).hover(p.hover)
        .on_press(on)
        .child(
            el().key("step-num").size(18.0, 18.0).radius(9.0).fill(p.accent_soft).center()
                .child(text(num.to_string()).font(ui::CAPTION).color(p.accent)),
        )
        .child(text(label.to_string()).font(type_::LABEL).color(p.text))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn detect_paths(st: &mut State) {
    let paths = detect_cod_paths();
    let txt = paths.iter().map(|x| x.display().to_string()).collect::<Vec<_>>().join("\n");
    st.paths_edit = TextEdit::new(&txt);
    st.push_scan_log(format!("Detected {} path(s)", paths.len()));
}

fn do_scan(st: &mut State) {
    if st.scanning { return; }
    let raw = st.paths_edit.text.to_string();
    let paths: Vec<PathBuf> = raw.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).map(PathBuf::from).collect();
    if paths.is_empty() {
        st.scan_status = "Add at least one folder.".to_string();
        st.push_scan_log("No paths provided.");
        return;
    }
    st.scanning = true;
    st.scan_status = "Scanning...".to_string();
    st.push_scan_log(format!("Scan started ({} path(s))", paths.len()));

    let out_dir = State::out_dir();
    let opts = ScanOptions {
        paths,
        output: out_dir.join("DMZ_SBMM_Reader_result.txt"),
        json_output: Some(out_dir.join("DMZ_SBMM_Reader_result.json")),
        zip_output: None,
        max_file_mb: st.max_mb,
        no_size_limit: st.no_size_limit,
        skip_binary: st.skip_binary,
        keywords: default_keywords(),
    };

    match scan(&opts) {
        Ok(r) => {
            st.scan_status = format!("Finished: scanned {} | skipped {} | errors {} | hits {}", r.summary.scanned, r.summary.skipped, r.summary.errors, r.summary.hits);
            st.push_scan_log(st.scan_status.clone());
            st.scan_report = Some(PathBuf::from(&r.output));
            st.scan_json = r.json_output.as_ref().map(PathBuf::from);
        }
        Err(e) => {
            st.scan_status = format!("Scan error: {e}");
            st.push_scan_log(st.scan_status.clone());
        }
    }
    st.scanning = false;
}

fn export_scan_zip(st: &mut State) {
    let Some(report) = st.scan_report.clone() else {
        st.push_scan_log("No scan report to export yet.");
        return;
    };
    let mut files = vec![report];
    if let Some(j) = &st.scan_json && j.exists() { files.push(j.clone()); }
    let Some(path) = rfd::FileDialog::new().set_file_name("DMZ_SBMM_Reader_results.zip").add_filter("ZIP", &["zip"]).save_file() else {
        st.push_scan_log("ZIP export cancelled.");
        return;
    };
    match crate::zip_export::zip_files(&files, &path) {
        Ok(_) => st.push_scan_log(format!("ZIP written: {}", path.display())),
        Err(e) => st.push_scan_log(format!("ZIP error: {e}")),
    }
}

fn probe_queue_start(st: &mut State) {
    st.ensure_probe();
    if st.probe_done {
        st.probe_done = false;
        st.start_net_sampler();
    }
    let result = st.probe_mut(|p| {
        let r = p.mark_queue_start();
        (r, p.match_number)
    });
    let msg = match result {
        Ok((Ok(_), n)) => format!("MATCH {n} queue started"),
        Ok((Err(e), _)) => format!("Probe error: {e}"),
        Err(e) => format!("Probe error: {e}"),
    };
    st.push_probe_log(msg);
    st.update_probe_status();
}

fn probe_queue_end(st: &mut State) {
    st.ensure_probe();
    let result = st.probe_mut(|p| {
        let n = p.match_number;
        (p.mark_queue_end(), n)
    });
    let msg = match result {
        Ok((Ok(sec), n)) => format!("MATCH {n} found ({sec:.1}s)"),
        Ok((Err(e), _)) => format!("Probe error: {e}"),
        Err(e) => format!("Probe error: {e}"),
    };
    st.push_probe_log(msg);
    st.update_probe_status();
}

fn probe_match_end(st: &mut State) {
    st.ensure_probe();
    let result = st.probe_mut(|p| {
        let n = p.match_number;
        (p.mark_match_end(), n)
    });
    let msg = match result {
        Ok((Ok(_), n)) => format!("MATCH {n} ended"),
        Ok((Err(e), _)) => format!("Probe error: {e}"),
        Err(e) => format!("Probe error: {e}"),
    };
    st.push_probe_log(msg);
    st.update_probe_status();
}

fn probe_stop(st: &mut State) {
    st.ensure_probe();
    if st.probe_done { st.push_probe_log("Probe already stopped."); return; }
    st.stop_net_sampler();
    let result = st.probe_ref(|p| p.write_report());
    let msg = match result {
        Ok(Ok(path)) => { st.probe_done = true; format!("Report written: {}", path.display()) }
        Ok(Err(e)) => format!("Report error: {e}"),
        Err(e) => format!("Probe error: {e}"),
    };
    st.push_probe_log(msg);
    st.update_probe_status();
}

fn export_probe_zip(st: &mut State) {
    st.ensure_probe();
    let Some(path) = rfd::FileDialog::new().set_file_name("DMZ_SBMM_Probe_results.zip").add_filter("ZIP", &["zip"]).save_file() else {
        st.push_probe_log("ZIP export cancelled.");
        return;
    };
    let result = st.probe_ref(|p| p.zip_outputs(&path));
    let msg = match result {
        Ok(Ok(_)) => format!("ZIP written: {}", path.display()),
        Ok(Err(e)) => format!("ZIP error: {e}"),
        Err(e) => format!("ZIP error: {e}"),
    };
    st.push_probe_log(msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_sampler_starts_and_stops() {
        let mut s = State::default();
        assert!(s.probe.is_some());
        assert!(s.net_sampler_thread.is_some(), "sampler thread should be running");
        assert!(s.probe_ref(|p| p.paths.network_csv.exists()).unwrap_or(false));
        s.stop_net_sampler();
        assert!(s.net_sampler_thread.is_none(), "sampler thread should be stopped");
    }
}
