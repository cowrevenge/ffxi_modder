//! Install ALL! (HD + Cow + Jelly)
//!
//! Finds the player's Pivot setup, downloads the mods from cowrevenge/ffxi_mod_files one at
//! a time (with progress and speed), unpacks them into Pivot's DATs folder, and manages
//! Pivot's overlay list (order and on/off) in config/pivot/pivot.ini.
#![windows_subsystem = "windows"]

use eframe::egui::{self, Color32, RichText, Stroke};
use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BASE_URL: &str = "https://github.com/cowrevenge/ffxi_mod_files/raw/main/downloads/";
/// lists downloads/ with each file's git blob sha: one request tells which downloads changed
const LIST_URL: &str = "https://api.github.com/repos/cowrevenge/ffxi_mod_files/contents/downloads?ref=main";

/// git's blob sha1 of a file: the same id GitHub lists, so a local file can be compared to it
fn blob_sha(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;
    let mut h = sha1_smol::Sha1::new();
    h.update(format!("blob {}\0", data.len()).as_bytes()); h.update(&data);
    Some(h.digest().to_string())
}

/// where the sha of the installed download is kept (inside the mod's own folder)
fn ver_file(dats: &Path, item: &Item) -> PathBuf { dats.join(item.folder).join(".installall").join(format!("{}.sha", item.file)) }

/// file name -> blob sha, from GitHub's listing of downloads/
fn remote_shas() -> Result<std::collections::HashMap<String, String>, String> {
    let url = std::env::var("INSTALLALL_LIST_URL").unwrap_or(LIST_URL.to_string());
    let body = agent().get(&url).set("User-Agent", "InstallALL").set("Accept", "application/vnd.github+json")
        .call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?;
    // small scan instead of a JSON library: each entry has "name": "..." then "sha": "..."
    let val = |s: &str, key: &str| -> Option<(String, usize)> {
        let k = s.find(&format!("\"{key}\""))?; let r = &s[k + key.len() + 2..];
        let a = r.find('"')? + 1; let b = r[a..].find('"')? + a;
        Some((r[a..b].to_string(), k + key.len() + 2 + b + 1))
    };
    let mut out = std::collections::HashMap::new(); let mut s = body.as_str();
    while let Some((name, e1)) = val(s, "name") {
        s = &s[e1..];
        if let Some((sha, e2)) = val(s, "sha") { out.insert(name, sha); s = &s[e2..]; } else { break; }
    }
    if out.is_empty() { return Err("empty listing".into()); }
    Ok(out)
}

// ---------------------------------------------------------------- catalogue

#[derive(Clone)]
struct Item {
    label: &'static str,
    file: &'static str,        // file under downloads/
    folder: &'static str,      // overlay folder the archive unpacks into
    marker: Option<&'static str>, // AutoHD races share a folder: per-race marker file
}

fn catalogue() -> Vec<Item> {
    let mut v = vec![
        Item { label: "Cow (Real Cows)", file: "Dhalmel-RealCows.7z", folder: "DhalmelCow-HD", marker: None },
        Item { label: "Shafty", file: "Dhalmel-Shafty.7z", folder: "DhalmelShaft-HD", marker: None },
        Item { label: "Jelly Marker", file: "JellyMarker.7z", folder: "JellyMarker", marker: None },
    ];
    for (r, label) in [("HumeM", "Hume (male)"), ("HumeF", "Hume (female)"), ("ElvaanM", "Elvaan (male)"),
                       ("ElvaanF", "Elvaan (female)"), ("Taru", "Tarutaru"), ("Mithra", "Mithra"), ("Galka", "Galka")] {
        v.push(Item {
            label,
            file: Box::leak(format!("CowMods-{r}.7z").into_boxed_str()),
            folder: "AutoHD",
            marker: Some(Box::leak(r.to_string().into_boxed_str())),
        });
    }
    v
}
const COW: usize = 0;
const SHAFTY: usize = 1;
const JELLY: usize = 2;
const RACES: std::ops::Range<usize> = 3..10;

// ---------------------------------------------------------------- game folder

/// case-insensitive child lookup (Linux/Wine folders keep Windows names in any case)
fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.exists() { return Some(direct); }
    fs::read_dir(dir).ok()?.flatten().map(|e| e.path())
        .find(|p| p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(name)).unwrap_or(false))
}

/// A game folder counts only if it holds polplugins/pivot.dll. Picking polplugins or
/// polplugins/DATs itself is accepted and walked up to the game folder.
fn pivot_root(p: &Path) -> Option<PathBuf> {
    let mut cand = vec![p.to_path_buf()];
    if let Some(a) = p.parent() { cand.push(a.to_path_buf()); if let Some(b) = a.parent() { cand.push(b.to_path_buf()); } }
    for c in cand {
        if let Some(pp) = child_ci(&c, "polplugins") {
            if child_ci(&pp, "pivot.dll").is_some() { return Some(c); }
        }
    }
    None
}

fn dats_dir(root: &Path) -> PathBuf {
    let pp = child_ci(root, "polplugins").unwrap_or(root.join("polplugins"));
    child_ci(&pp, "DATs").unwrap_or(pp.join("DATs"))
}

fn pivot_ini(root: &Path) -> PathBuf {
    let cfg = child_ci(root, "config").unwrap_or(root.join("config"));
    let pv = child_ci(&cfg, "pivot").unwrap_or(cfg.join("pivot"));
    child_ci(&pv, "pivot.ini").unwrap_or(pv.join("pivot.ini"))
}

fn autodetect() -> Option<PathBuf> {
    let mut bases: Vec<PathBuf> = vec![];
    if cfg!(windows) {
        for d in b'C'..=b'H' {
            let drive = PathBuf::from(format!("{}:\\", d as char));
            if !drive.exists() { continue; }
            bases.push(drive.clone());
            bases.push(drive.join("Games"));
            bases.push(drive.join("Program Files (x86)"));
            bases.push(drive.join("Program Files"));
        }
        if let Ok(h) = std::env::var("USERPROFILE") {
            let h = PathBuf::from(h);
            for s in ["", "Desktop", "Documents", "Games", "Downloads"] { bases.push(h.join(s)); }
        }
    } else if let Ok(h) = std::env::var("HOME") {
        let h = PathBuf::from(h);
        let mut prefixes = vec![h.join(".wine")];
        for g in [h.join("Games"), h.join(".local/share/lutris/prefixes")] {
            if let Ok(rd) = fs::read_dir(&g) { prefixes.extend(rd.flatten().map(|e| e.path())); }
        }
        for s in [h.join(".local/share/Steam/steamapps/compatdata"), h.join(".steam/steam/steamapps/compatdata")] {
            if let Ok(rd) = fs::read_dir(&s) { prefixes.extend(rd.flatten().map(|e| e.path().join("pfx"))); }
        }
        for p in prefixes {
            let c = p.join("drive_c");
            bases.push(c.clone());
            bases.push(c.join("Program Files (x86)"));
            bases.push(c.join("Program Files"));
            bases.push(c.join("Games"));
        }
        bases.push(h.clone());
    }
    // the base itself, its children, and its grandchildren (e.g. ...\PlayOnline\Ashita)
    for b in bases {
        if let Some(r) = pivot_root(&b) { if r == b { return Some(r); } }
        let Ok(rd) = fs::read_dir(&b) else { continue };
        let kids: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        for k in &kids { if child_ci(k, "polplugins").map(|pp| child_ci(&pp, "pivot.dll").is_some()).unwrap_or(false) { return Some(k.clone()); } }
        for k in &kids {
            if let Ok(rd2) = fs::read_dir(k) {
                for g in rd2.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
                    if child_ci(&g, "polplugins").map(|pp| child_ci(&pp, "pivot.dll").is_some()).unwrap_or(false) { return Some(g); }
                }
            }
        }
    }
    None
}

fn settings_file() -> PathBuf {
    std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("InstallALL.cfg"))).unwrap_or("InstallALL.cfg".into())
}

// ---------------------------------------------------------------- pivot.ini

struct PivotIni { path: PathBuf, before: Vec<String>, after: Vec<String>, overlays: Vec<String> }

impl PivotIni {
    fn load(path: &Path) -> PivotIni {
        let text = fs::read_to_string(path).unwrap_or_default();
        let (mut before, mut after, mut items) = (vec![], vec![], vec![]);
        let mut state = 0; // 0 before [overlays], 1 inside, 2 after
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                if t.eq_ignore_ascii_case("[overlays]") { state = 1; continue; }
                if state == 1 { state = 2; }
            }
            match state {
                0 => before.push(line.to_string()),
                1 => {
                    if t.starts_with(';') || t.is_empty() { continue; }
                    if let Some((k, v)) = t.split_once('=') {
                        if let Ok(i) = k.trim().parse::<i64>() { if !v.trim().is_empty() { items.push((i, v.trim().to_string())); } }
                    }
                }
                _ => after.push(line.to_string()),
            }
        }
        items.sort_by_key(|x| x.0);
        PivotIni { path: path.to_path_buf(), before, after, overlays: items.into_iter().map(|x| x.1).collect() }
    }
    fn save(&self) -> std::io::Result<()> {
        if let Some(d) = self.path.parent() { fs::create_dir_all(d)?; }
        let mut s = String::new();
        for l in &self.before { s += l; s += "\r\n"; }
        s += "[overlays]\r\n";
        for (i, o) in self.overlays.iter().enumerate() { s += &format!("{i}={o}\r\n"); }
        if !self.after.is_empty() { s += "\r\n"; }
        for l in &self.after { s += l; s += "\r\n"; }
        let tmp = self.path.with_extension("ini.tmp");
        fs::write(&tmp, s)?;
        fs::rename(tmp, &self.path)
    }
}

fn is_dhalmel(n: &str) -> bool { n.to_ascii_lowercase().starts_with("dhalmel") }
fn rank(n: &str) -> Option<u8> {
    let l = n.to_ascii_lowercase();
    if l.starts_with("dhalmel") { Some(0) } else if l == "jellymarker" { Some(1) } else if l == "autohd" { Some(2) } else { None }
}

/// put a freshly installed overlay where it belongs: Dhalmel, JellyMarker, AutoHD, then the rest
fn place_overlay(ov: &mut Vec<String>, name: &str) {
    if is_dhalmel(name) { ov.retain(|o| !is_dhalmel(o) || o.eq_ignore_ascii_case(name)); }
    if ov.iter().any(|o| o.eq_ignore_ascii_case(name)) { return; }
    let r = rank(name).unwrap_or(9);
    let at = ov.iter().position(|o| rank(o).map(|x| x > r).unwrap_or(true)).unwrap_or(ov.len());
    ov.insert(at, name.to_string());
}

// ---------------------------------------------------------------- downloads

#[derive(Clone, PartialEq)]
enum Phase { Connecting, Downloading, Unpacking, Done, Failed(String) }

#[derive(Clone)]
struct Progress { item: usize, phase: Phase, got: u64, total: Option<u64>, speed: f64 }

fn human(b: f64) -> String {
    if b >= 1048576.0 { format!("{:.1} MB", b / 1048576.0) } else { format!("{:.0} KB", b / 1024.0) }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().try_proxy_from_env(true).timeout_connect(Duration::from_secs(20)).timeout_read(Duration::from_secs(60)).build()
}

fn run_job(item: Item, idx: usize, dats: PathBuf, prog: Arc<Mutex<Progress>>) {
    let set = |f: &dyn Fn(&mut Progress)| { if let Ok(mut p) = prog.lock() { f(&mut p) } };
    let fail = |msg: String| set(&|p: &mut Progress| p.phase = Phase::Failed(msg.clone()));
    let work = dats.join(".installall");
    let _ = fs::create_dir_all(&work);
    let part = work.join(format!("{}.part", item.file));
    // INSTALLALL_BASE_URL: point at a test server (testing only)
    let base = std::env::var("INSTALLALL_BASE_URL").unwrap_or(BASE_URL.to_string());
    let url = format!("{base}{}", item.file);

    let resp = match agent().get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => return fail(format!("Download not found: {} is not on GitHub.", item.file)),
        Err(ureq::Error::Status(c, _)) => return fail(format!("GitHub answered {c} for {}. Try again later.", item.file)),
        Err(e) => return fail(format!("Could not reach GitHub: {e}")),
    };
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
    set(&|p: &mut Progress| { p.phase = Phase::Downloading; p.total = total; });
    let mut out = match fs::File::create(&part) { Ok(f) => f, Err(e) => return fail(format!("Cannot write to {}: {e}", work.display())) };
    let mut rd = resp.into_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let (mut got, mut win_bytes, mut win_t) = (0u64, 0u64, Instant::now());
    loop {
        let n = match rd.read(&mut buf) { Ok(0) => break, Ok(n) => n, Err(e) => { drop(out); let _ = fs::remove_file(&part); return fail(format!("Download dropped: {e}")); } };
        if let Err(e) = out.write_all(&buf[..n]) { return fail(format!("Disk write failed: {e}")); }
        got += n as u64; win_bytes += n as u64;
        let dt = win_t.elapsed().as_secs_f64();
        if dt >= 0.5 {
            let sp = win_bytes as f64 / dt;
            set(&|p: &mut Progress| { p.got = got; p.speed = if p.speed == 0.0 { sp } else { p.speed * 0.6 + sp * 0.4 }; });
            win_bytes = 0; win_t = Instant::now();
        } else {
            let first = win_bytes as f64 / dt.max(0.05);
            set(&|p: &mut Progress| { p.got = got; if p.speed == 0.0 && dt > 0.05 { p.speed = first; } });
        }
    }
    drop(out);
    if let Some(t) = total { if got != t { let _ = fs::remove_file(&part); return fail(format!("Download incomplete ({} of {}). Try again.", human(got as f64), human(t as f64))); } }

    // unpack into a staging folder first: nothing half-unpacked ever reaches the DATs folder
    set(&|p: &mut Progress| p.phase = Phase::Unpacking);
    let stage = work.join(format!("stage-{idx}"));
    let _ = fs::remove_dir_all(&stage);
    if let Err(e) = sevenz_rust::decompress_file(&part, &stage) {
        let _ = fs::remove_dir_all(&stage); let _ = fs::remove_file(&part);
        return fail(format!("The download is damaged ({e}). Try again."));
    }
    let sha = blob_sha(&part);
    if let Err(e) = merge(&stage, &dats) { return fail(format!("Could not copy files into {}: {e}", dats.display())); }
    if let Some(sha) = sha {
        let vf = ver_file(&dats, &item);
        if let Some(d) = vf.parent() { let _ = fs::create_dir_all(d); }
        let _ = fs::write(vf, sha);
    }
    let _ = fs::remove_dir_all(&stage); let _ = fs::remove_file(&part);
    if let Some(m) = item.marker {
        let md = dats.join(item.folder).join(".installall");
        let _ = fs::create_dir_all(&md); let _ = fs::write(md.join(m), b"installed");
    }
    set(&|p: &mut Progress| { p.phase = Phase::Done; p.got = p.total.unwrap_or(got); });
}

fn merge(from: &Path, to: &Path) -> std::io::Result<()> {
    for e in fs::read_dir(from)? {
        let e = e?; let src = e.path(); let dst = to.join(e.file_name());
        if src.is_dir() { fs::create_dir_all(&dst)?; merge(&src, &dst)?; }
        else { if dst.exists() { fs::remove_file(&dst)?; } if fs::rename(&src, &dst).is_err() { fs::copy(&src, &dst)?; } }
    }
    Ok(())
}

// ---------------------------------------------------------------- app

#[derive(PartialEq)]
enum Screen { Main, Pivot }

struct Browser { dir: PathBuf, sel: Option<PathBuf> }

struct App {
    items: Vec<Item>,
    root: Option<PathBuf>,
    root_note: String,
    browser: Option<Browser>,
    queue: VecDeque<usize>,
    active: Option<Arc<Mutex<Progress>>>,
    last: Vec<Option<Phase>>,  // last finished result per item
    screen: Screen,
    pivot: Option<PivotIni>,
    pivot_inactive: Vec<String>,
    pivot_dirty: bool,
    pivot_msg: Option<(bool, String)>,
    on: Vec<String>,           // overlays switched on in pivot.ini (re-read every 2 s)
    remote: Arc<Mutex<Option<std::collections::HashMap<String, String>>>>,  // GitHub's sha per download
    on_read: Instant,
}

impl App {
    fn new() -> App {
        let items = catalogue();
        let n = items.len();
        let saved = fs::read_to_string(settings_file()).ok().map(|s| PathBuf::from(s.trim())).and_then(|p| pivot_root(&p));
        let env = std::env::var("INSTALLALL_ROOT").ok().map(PathBuf::from).and_then(|p| pivot_root(&p));
        let (root, note) = match env.or(saved) {
            Some(r) => (Some(r), "Pivot found.".to_string()),
            None => match autodetect() {
                Some(r) => (Some(r), "Pivot found automatically.".to_string()),
                None => (None, "Pivot not found. Use Browse to pick your game folder (the one with polplugins).".to_string()),
            },
        };
        // leftovers from a download that was interrupted (game folder was closed mid-way)
        if let Some(r) = &root { let _ = fs::remove_dir_all(dats_dir(r).join(".installall")); }
        App { items, root, root_note: note, browser: None, queue: VecDeque::new(), active: None, last: vec![None; n],
              screen: Screen::Main, pivot: None, pivot_inactive: vec![], pivot_dirty: false, pivot_msg: None,
              on: vec![], on_read: Instant::now() - std::time::Duration::from_secs(10), remote: Arc::new(Mutex::new(None)) }
    }

    fn refresh_on(&mut self) {
        self.on = self.root.as_ref().map(|r| PivotIni::load(&pivot_ini(r)).overlays).unwrap_or_default();
        self.on_read = Instant::now();
    }

    /// ask GitHub (in the background) which downloads changed
    fn check_updates(&self) {
        let r = self.remote.clone();
        std::thread::spawn(move || { if let Ok(m) = remote_shas() { *r.lock().unwrap() = Some(m); } });
    }

    /// downloaded, and GitHub has a different file now (or we can't tell which one is installed)
    fn update_avail(&self, i: usize) -> bool {
        let Some(root) = &self.root else { return false };
        if !self.installed(i) { return false; }
        let it = &self.items[i];
        let Some(remote) = self.remote.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.get(it.file).cloned())) else { return false };
        fs::read_to_string(ver_file(&dats_dir(root), it)).map(|s| s.trim() != remote).unwrap_or(true)
    }

    fn is_on(&self, folder: &str) -> bool { self.on.iter().any(|o| o.eq_ignore_ascii_case(folder)) }

    /// switch an overlay on or off in pivot.ini (on: put in its place, other Dhalmel goes off)
    fn set_on(&mut self, folder: &str, on: bool) {
        let Some(r) = &self.root else { return };
        let mut ini = PivotIni::load(&pivot_ini(r));
        if on { place_overlay(&mut ini.overlays, folder); } else { ini.overlays.retain(|o| !o.eq_ignore_ascii_case(folder)); }
        let _ = ini.save();
        self.refresh_on();
    }

    fn installed(&self, i: usize) -> bool {
        let Some(r) = &self.root else { return false };
        let d = dats_dir(r).join(self.items[i].folder);
        match self.items[i].marker { Some(m) => d.join(".installall").join(m).exists(), None => d.is_dir() }
    }

    fn enqueue(&mut self, i: usize) {
        if self.root.is_none() { return; }
        let running = self.active.as_ref().map(|a| a.lock().map(|p| p.item == i).unwrap_or(false)).unwrap_or(false);
        if !running && !self.queue.contains(&i) { self.queue.push_back(i); self.last[i] = None; }
    }

    fn pump(&mut self) {
        // finished job: record result, update pivot.ini, start the next one
        if let Some(a) = &self.active {
            let p = a.lock().unwrap().clone();
            if matches!(p.phase, Phase::Done | Phase::Failed(_)) {
                if p.phase == Phase::Done {
                    if let Some(r) = &self.root {
                        let mut ini = PivotIni::load(&pivot_ini(r));
                        place_overlay(&mut ini.overlays, self.items[p.item].folder);
                        let _ = ini.save();
                        if self.pivot.is_some() { self.open_pivot(); }
                        self.refresh_on();
                    }
                }
                self.last[p.item] = Some(p.phase.clone());
                self.active = None;
            }
        }
        if self.active.is_none() {
            if let (Some(i), Some(r)) = (self.queue.pop_front(), self.root.clone()) {
                let prog = Arc::new(Mutex::new(Progress { item: i, phase: Phase::Connecting, got: 0, total: None, speed: 0.0 }));
                self.active = Some(prog.clone());
                let item = self.items[i].clone();
                std::thread::spawn(move || run_job(item, i, dats_dir(&r), prog));
            }
        }
    }

    fn open_pivot(&mut self) {
        let Some(r) = &self.root else { return };
        let ini = PivotIni::load(&pivot_ini(r));
        let mut inactive: Vec<String> = fs::read_dir(dats_dir(r)).map(|rd| rd.flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| !n.starts_with('.') && !ini.overlays.iter().any(|o| o.eq_ignore_ascii_case(n)))
            .collect()).unwrap_or_default();
        inactive.sort_by_key(|s| s.to_lowercase());
        self.pivot = Some(ini);
        self.pivot_inactive = inactive;
        self.pivot_dirty = false;
    }

    // ------------------------------------------------ main screen
    fn ui_main(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Game Files Folder").strong());
            ui.horizontal(|ui| {
                let txt = self.root.as_ref().map(|r| r.display().to_string()).unwrap_or("not found".into());
                ui.add(egui::Label::new(RichText::new(txt).monospace()).truncate());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Browse...").clicked() {
                        let start = self.root.clone().unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }));
                        self.browser = Some(Browser { dir: start, sel: None });
                    }
                });
            });
            let ok = self.root.is_some();
            ui.colored_label(if ok { Color32::from_rgb(90, 200, 120) } else { Color32::from_rgb(235, 110, 90) }, &self.root_note);
        });

        ui.add_space(8.0);
        let enabled = self.root.is_some();
        if self.on_read.elapsed().as_secs_f32() > 2.0 { self.refresh_on(); }
        let dhalmel_note = "Cow and Shafty can both be downloaded, but only one can be enabled. Enabling one disables the other. Close the game before enabling or disabling.";
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Mods").strong());
            egui::Grid::new("mods").num_columns(3).spacing([16.0, 8.0]).min_col_width(80.0).show(ui, |ui| {
                for i in [COW, SHAFTY, JELLY] {
                    self.row(ui, i, enabled, if i == JELLY { "own folder: JellyMarker" } else { "replaces the Dhalmel" });
                    ui.end_row();
                }
            });
            ui.small(dhalmel_note);
        });

        ui.add_space(8.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let id = ui.make_persistent_id("autohd_races");
            let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
            state.show_header(ui, |ui| {
                ui.label(RichText::new("AutoHD Gear").strong());
                let n = RACES.filter(|&i| self.installed(i)).count();
                ui.weak(format!("{n}/7 races downloaded  (Bastok Pride included)"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if n > 0 {
                        let on = self.is_on("AutoHD");
                        if ui.add_enabled(enabled, egui::Button::new(if on { "Disable" } else { "Enable" }).min_size(egui::vec2(90.0, 0.0))).clicked() { self.set_on("AutoHD", !on); }
                        if on { ui.colored_label(Color32::from_rgb(90, 200, 120), "enabled"); } else { ui.weak("disabled"); }
                    }
                    let todo: Vec<usize> = RACES.filter(|&i| !self.installed(i) || self.update_avail(i)).collect();
                    let label = if n < 7 { "Download all races" } else { "Update all races" };
                    if !todo.is_empty() && ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        for i in todo { self.enqueue(i); }
                    }
                });
            }).body(|ui| {
                egui::Grid::new("races").num_columns(3).spacing([16.0, 6.0]).min_col_width(80.0).show(ui, |ui| {
                    for i in RACES { self.row(ui, i, enabled, ""); ui.end_row(); }
                });
            });
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(enabled, egui::Button::new(RichText::new("Open Pivot").strong()).min_size(egui::vec2(140.0, 30.0))).clicked() {
                self.open_pivot(); self.screen = Screen::Pivot;
            }
            ui.weak("change overlay order, switch overlays on or off");
        });
    }

    fn row(&mut self, ui: &mut egui::Ui, i: usize, enabled: bool, note: &str) {
        let it = &self.items[i];
        ui.horizontal(|ui| { ui.label(RichText::new(it.label).size(15.0)); if !note.is_empty() { ui.weak(note); } });
        // status
        let active = self.active.as_ref().and_then(|a| a.lock().ok().map(|p| p.clone())).filter(|p| p.item == i);
        let waiting = self.queue.iter().position(|&q| q == i);
        ui.horizontal(|ui| {
            ui.set_min_width(300.0);
            if let Some(p) = active {
                match &p.phase {
                    Phase::Connecting => { ui.spinner(); ui.label("connecting..."); }
                    Phase::Downloading => {
                        let frac = p.total.map(|t| p.got as f32 / t.max(1) as f32).unwrap_or(0.0);
                        ui.add(egui::ProgressBar::new(frac).desired_width(150.0).show_percentage());
                        ui.label(format!("{} / {}  {}/s", human(p.got as f64), p.total.map(|t| human(t as f64)).unwrap_or("?".into()), human(p.speed)));
                    }
                    Phase::Unpacking => { ui.spinner(); ui.label("unpacking..."); }
                    _ => {}
                }
            } else if let Some(pos) = waiting {
                ui.colored_label(Color32::from_rgb(230, 190, 90), format!("waiting on the others ({} ahead)", pos + 1 - (self.active.is_none() as usize)));
            } else if let Some(Phase::Failed(m)) = &self.last[i] {
                ui.colored_label(Color32::from_rgb(235, 110, 90), m);
            } else if self.installed(i) {
                let shared = self.items[i].marker.is_some();   // AutoHD races: one folder, switched on in the header
                if shared || self.is_on(self.items[i].folder) {
                    ui.colored_label(Color32::from_rgb(90, 200, 120), if shared { "\u{2714} downloaded" } else { "\u{2714} downloaded, enabled" });
                } else {
                    ui.weak("\u{2714} downloaded, disabled");
                }
                if self.update_avail(i) { ui.colored_label(Color32::from_rgb(230, 190, 90), "update available"); }
            } else {
                ui.weak("not downloaded");
            }
        });
        let busy = self.queue.contains(&i) || self.active.as_ref().map(|a| a.lock().map(|p| p.item == i).unwrap_or(false)).unwrap_or(false);
        let size = egui::vec2(90.0, 0.0);
        if matches!(self.last[i], Some(Phase::Failed(_))) || !self.installed(i) || busy {
            let label = if matches!(self.last[i], Some(Phase::Failed(_))) { "Retry" } else { "Download" };
            if ui.add_enabled(enabled && !busy, egui::Button::new(label).min_size(size)).clicked() { self.enqueue(i); }
        } else {
            let upd = self.update_avail(i);
            ui.horizontal(|ui| {
                if self.items[i].marker.is_none() {
                    let folder = self.items[i].folder;
                    let on = self.is_on(folder);
                    if ui.add_enabled(enabled, egui::Button::new(if on { "Disable" } else { "Enable" }).min_size(size)).clicked() { self.set_on(folder, !on); }
                }
                if upd && ui.add_enabled(enabled, egui::Button::new(RichText::new("Update").color(Color32::from_rgb(230, 190, 90))).min_size(size)).clicked() { self.enqueue(i); }
            });
        }
    }

    // ------------------------------------------------ pivot screen
    fn ui_pivot(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("< Back").clicked() { self.screen = Screen::Main; }
            ui.heading("Pivot overlays");
        });
        let Some(ini) = &mut self.pivot else { return };
        ui.weak(format!("{}", ini.path.display()));
        ui.add_space(4.0);
        ui.label("The first overlay in the list that has a file wins. Drag rows to change the order.");

        // warnings
        let ov = ini.overlays.clone();
        let pos = |n: &str| ov.iter().position(|o| o.eq_ignore_ascii_case(n));
        let mut warns = vec![];
        if ov.iter().filter(|o| is_dhalmel(o)).count() > 1 { warns.push("Two Dhalmel mods are on: only the higher one shows. Switch one off.".to_string()); }
        if let Some(a) = pos("AutoHD") {
            for other in ["NextHD", "EraDATs", "XICombinedMaps-2048", "XiView-Standard", "XiView-Widescreen"] {
                if let Some(b) = pos(other) { if b < a && (other == "NextHD" || other == "EraDATs") { warns.push(format!("AutoHD is below {other}: {other}'s copies of the gear win. Drag AutoHD above it.")); } }
            }
        }
        if let Some(j) = pos("JellyMarker") { if let Some(b) = pos("NextHD") { if b < j { warns.push("JellyMarker is below NextHD: drag it above.".into()); } } }
        for w in warns { ui.colored_label(Color32::from_rgb(235, 170, 70), format!("\u{26A0} {w}")); }
        ui.add_space(6.0);

        // active list with drag and drop
        ui.label(RichText::new("Active (top = highest priority)").strong());
        let mut move_req: Option<(usize, usize)> = None;
        let mut off_req: Option<usize> = None;
        let dragged = egui::DragAndDrop::payload::<usize>(ui.ctx()).map(|a| *a);
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let released = ui.input(|i| i.pointer.any_released());
        let mut rects = vec![];
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if ini.overlays.is_empty() { ui.weak("no active overlays"); }
            for (k, name) in ini.overlays.iter().enumerate() {
                let r = ui.horizontal(|ui| {
                    ui.dnd_drag_source(egui::Id::new(("ov", k, name.as_str())), k, |ui| {
                        ui.label(RichText::new("\u{2630}").size(16.0));
                        ui.label(RichText::new(format!("{k:02}")).monospace());
                        ui.label(RichText::new(name).size(15.0));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("switch off").clicked() { off_req = Some(k); }
                        if ui.add_enabled(k + 1 < ov.len(), egui::Button::new("down").small()).clicked() { move_req = Some((k, k + 1)); }
                        if ui.add_enabled(k > 0, egui::Button::new("up").small()).clicked() { move_req = Some((k, k - 1)); }
                    });
                }).response.rect;
                rects.push(r);
            }
        });
        // drop target: insertion line between rows
        if let (Some(from), Some(p)) = (dragged, pointer) {
            if let (Some(first), Some(last)) = (rects.first(), rects.last()) {
                if p.x >= first.left() && p.x <= first.right() && p.y >= first.top() - 20.0 && p.y <= last.bottom() + 20.0 {
                    let slot = rects.iter().position(|r| p.y < r.center().y).unwrap_or(rects.len());
                    let y = if slot < rects.len() { rects[slot].top() - 2.0 } else { last.bottom() + 2.0 };
                    ui.painter().hline(first.left()..=first.right(), y, Stroke::new(3.0_f32, Color32::from_rgb(80, 170, 255)));
                    if released {
                        let to = if slot > from { slot - 1 } else { slot };
                        move_req = Some((from, to));
                    }
                }
            }
        }
        if let Some((a, b)) = move_req { if a != b && b < ini.overlays.len() { let x = ini.overlays.remove(a); ini.overlays.insert(b, x); self.pivot_dirty = true; } }
        if let Some(k) = off_req { let x = ini.overlays.remove(k); self.pivot_inactive.push(x); self.pivot_inactive.sort_by_key(|s| s.to_lowercase()); self.pivot_dirty = true; }

        ui.add_space(6.0);
        ui.label(RichText::new("Not active").strong());
        let mut on_req = None;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.pivot_inactive.is_empty() { ui.weak("every overlay folder is active"); }
            for (k, name) in self.pivot_inactive.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).size(15.0).color(ui.visuals().weak_text_color()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { if ui.button("switch on").clicked() { on_req = Some(k); } });
                });
            }
        });
        if let Some(k) = on_req {
            let x = self.pivot_inactive.remove(k);
            let ini = self.pivot.as_mut().unwrap();
            place_overlay(&mut ini.overlays, &x);
            if is_dhalmel(&x) {
                // the other Dhalmel was switched off by place_overlay: list it as inactive
                let act: Vec<String> = ini.overlays.clone();
                if let Ok(rd) = fs::read_dir(dats_dir(self.root.as_ref().unwrap())) {
                    for e in rd.flatten() { let n = e.file_name().to_string_lossy().to_string();
                        if is_dhalmel(&n) && !act.iter().any(|o| o.eq_ignore_ascii_case(&n)) && !self.pivot_inactive.contains(&n) { self.pivot_inactive.push(n); } }
                }
            }
            self.pivot_dirty = true;
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(self.pivot_dirty, egui::Button::new(RichText::new("Save").strong()).min_size(egui::vec2(110.0, 30.0))).clicked() {
                self.pivot_msg = Some(match self.pivot.as_ref().unwrap().save() {
                    Ok(()) => { self.pivot_dirty = false; (true, "Saved. Restart the game to use the new order.".into()) }
                    Err(e) => (false, format!("Could not save: {e}")),
                });
            }
            if ui.button("Undo changes").clicked() { self.open_pivot(); self.pivot_msg = None; }
            if self.pivot_dirty { ui.colored_label(Color32::from_rgb(230, 190, 90), "unsaved changes"); }
        });
        if let Some((ok, m)) = &self.pivot_msg { ui.colored_label(if *ok { Color32::from_rgb(90, 200, 120) } else { Color32::from_rgb(235, 110, 90) }, m); }
        ui.weak("Close the game before saving: Pivot rewrites this file while the game runs.");
    }

    // ------------------------------------------------ folder browser
    fn ui_browser(&mut self, ctx: &egui::Context) {
        let Some(b) = &mut self.browser else { return };
        let mut close = false; let mut pick: Option<PathBuf> = None;
        egui::Window::new("Pick your game folder").collapsible(false).resizable(true).default_size([560.0, 420.0]).show(ctx, |ui| {
            ui.label("Pick the folder that holds polplugins (for example C:\\PhoenixXI). Folders with Pivot show a \u{2714}.");
            ui.horizontal(|ui| {
                if ui.button("Up one folder").clicked() { if let Some(p) = b.dir.parent() { b.dir = p.to_path_buf(); b.sel = None; } }
                if cfg!(windows) {
                    for d in b'C'..=b'H' { let dr = PathBuf::from(format!("{}:\\", d as char)); if dr.exists() && ui.small_button(format!("{}:", d as char)).clicked() { b.dir = dr; } }
                } else if let Ok(h) = std::env::var("HOME") { if ui.small_button("Home").clicked() { b.dir = h.into(); } }
                ui.add(egui::Label::new(RichText::new(b.dir.display().to_string()).monospace()).truncate());
            });
            ui.separator();
            let mut dirs: Vec<PathBuf> = fs::read_dir(&b.dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
            dirs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default());
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for d in dirs {
                    let name = d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    if name.starts_with('.') && !name.eq_ignore_ascii_case(".wine") { continue; }
                    let ok = child_ci(&d, "polplugins").map(|pp| child_ci(&pp, "pivot.dll").is_some()).unwrap_or(false);
                    let txt = if ok { RichText::new(format!("\u{2714} {name}")).color(Color32::from_rgb(90, 200, 120)) } else { RichText::new(format!("\u{1F4C1} {name}")) };
                    let r = ui.selectable_label(b.sel.as_ref() == Some(&d), txt);
                    if r.double_clicked() { b.dir = d.clone(); b.sel = None; } else if r.clicked() { b.sel = Some(d.clone()); }
                }
            });
            ui.separator();
            let here = b.sel.as_ref().and_then(|s| pivot_root(s)).or_else(|| pivot_root(&b.dir));
            match &here {
                Some(r) => { ui.colored_label(Color32::from_rgb(90, 200, 120), format!("\u{2714} Pivot found: {}", r.display())); }
                None => { ui.colored_label(Color32::from_rgb(235, 110, 90), "This is not a Pivot folder (no polplugins\\pivot.dll). Click a \u{2714} folder, or double-click folders to open them."); }
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(here.is_some(), egui::Button::new(RichText::new("Use this folder").strong())).clicked() { pick = here.clone(); }
                if ui.button("Cancel").clicked() { close = true; }
            });
        });
        if let Some(r) = pick {
            let _ = fs::write(settings_file(), r.display().to_string());
            self.root = Some(r); self.root_note = "Pivot found.".into(); self.browser = None;
        } else if close { self.browser = None; }
    }
}

struct Shell { app: App, frame: u64, shot: Option<String>, t0: Instant, fired: bool }

impl eframe::App for Shell {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        let a = &mut self.app;
        a.pump();
        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Install ALL!").size(26.0).strong().color(Color32::from_rgb(240, 200, 90)));
                ui.label(RichText::new("(HD + Cow + Jelly)").size(16.0));
            });
            ui.add_space(4.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let busy = a.active.is_some();
                let q = a.queue.len();
                if busy { ui.spinner(); }
                ui.label(if busy { format!("downloading, {q} waiting") } else if q > 0 { format!("{q} waiting") } else { "idle".into() });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.weak("github.com/cowrevenge/ffxi_mod_files"); });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match a.screen { Screen::Main => a.ui_main(ui), Screen::Pivot => a.ui_pivot(ui) }
            });
        });
        a.ui_browser(ctx);
        if a.active.is_some() || !a.queue.is_empty() { ctx.request_repaint_after(Duration::from_millis(100)); }
        else { ctx.request_repaint_after(Duration::from_secs(1)); }   // pivot.ini and update check land without input

        // INSTALLALL_SHOT=out.ppm[,pivot][,browse][,queue:0:5]: render a screenshot and exit (docs/tests)
        if let Some(spec) = self.shot.clone() {
            self.frame += 1;
            if self.frame == 2 {
                for part in spec.split(',').skip(1) {
                    if part == "pivot" { a.open_pivot(); a.screen = Screen::Pivot; }
                    if part == "browse" { a.browser = Some(Browser { dir: a.root.clone().and_then(|r| r.parent().map(|p| p.to_path_buf())).unwrap_or("/".into()), sel: None }); }
                    if let Some(q) = part.strip_prefix("queue:") { for i in q.split(':').filter_map(|x| x.parse().ok()) { a.enqueue(i); } }
                }
            }
            let wait = std::env::var("INSTALLALL_SHOT_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0f64);
            if !self.fired && self.frame > 4 && self.t0.elapsed().as_secs_f64() >= wait {
                self.fired = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            ctx.input(|i| for ev in &i.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    let path = spec.split(',').next().unwrap();
                    let mut f = fs::File::create(path).unwrap();
                    let _ = write!(f, "P6\n{} {}\n255\n", image.size[0], image.size[1]);
                    for p in &image.pixels { let _ = f.write_all(&[p.r(), p.g(), p.b()]); }
                    std::process::exit(0);
                }
            });
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 720.0]).with_min_inner_size([560.0, 480.0]).with_title("Install ALL! (HD + Cow + Jelly)"),
        ..Default::default()
    };
    let shot = std::env::var("INSTALLALL_SHOT").ok();
    eframe::run_native("Install ALL! (HD + Cow + Jelly)", opts, Box::new(move |cc| {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|s| { s.spacing.button_padding = egui::vec2(10.0, 5.0); });
        Ok(Box::new(Shell { app: { let a = App::new(); a.check_updates(); a }, frame: 0, shot, t0: Instant::now(), fired: false }))
    }))
}
