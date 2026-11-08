// UnstuckSpotify — fixes a Spotify hang/crash by clearing its cache (Step 1)
// or doing a full reset (Step 2). Run on macOS, Windows, or Linux.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// Spotify brand palette
const SPOTIFY_BLACK: egui::Color32 = egui::Color32::from_rgb(0x12, 0x12, 0x12);
const SPOTIFY_PANEL: egui::Color32 = egui::Color32::from_rgb(0x18, 0x18, 0x18);
const SPOTIFY_CARD: egui::Color32 = egui::Color32::from_rgb(0x24, 0x24, 0x24);
const SPOTIFY_GREEN: egui::Color32 = egui::Color32::from_rgb(0x1D, 0xB9, 0x54);
const SPOTIFY_GREEN_DIM: egui::Color32 = egui::Color32::from_rgb(0x07, 0x2E, 0x15);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0xB3, 0xB3, 0xB3);

#[derive(Clone, Copy)]
enum Action {
    Step1,
    Step2,
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Unstuck Spotify")
            .with_inner_size([560.0, 640.0])
            .with_min_inner_size([460.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Unstuck Spotify",
        options,
        Box::new(|cc| {
            apply_spotify_style(&cc.egui_ctx);
            Ok(Box::new(UnstuckApp::new()))
        }),
    )
    .expect("failed to start Unstuck Spotify");
}

fn apply_spotify_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;

    v.dark_mode = true;
    v.panel_fill = SPOTIFY_BLACK;
    v.window_fill = SPOTIFY_BLACK;
    v.extreme_bg_color = SPOTIFY_BLACK;
    v.faint_bg_color = SPOTIFY_PANEL;

    v.widgets.noninteractive.bg_fill = SPOTIFY_PANEL;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, SPOTIFY_GREEN_DIM);

    v.widgets.inactive.bg_fill = SPOTIFY_CARD;
    v.widgets.inactive.weak_bg_fill = SPOTIFY_CARD;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);

    v.widgets.hovered.bg_fill = SPOTIFY_GREEN_DIM;
    v.widgets.hovered.weak_bg_fill = SPOTIFY_GREEN_DIM;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, SPOTIFY_GREEN);

    v.widgets.active.bg_fill = SPOTIFY_GREEN;
    v.widgets.active.weak_bg_fill = SPOTIFY_GREEN;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, SPOTIFY_BLACK);

    v.selection.bg_fill = SPOTIFY_GREEN;
    v.selection.stroke = egui::Stroke::new(1.0_f32, SPOTIFY_BLACK);

    v.override_text_color = Some(TEXT_PRIMARY);
    v.hyperlink_color = SPOTIFY_GREEN;

    // Rounded corners everywhere
    let rounding = egui::Rounding::same(10.0);
    v.widgets.noninteractive.rounding = rounding;
    v.widgets.inactive.rounding = rounding;
    v.widgets.hovered.rounding = rounding;
    v.widgets.active.rounding = rounding;
    v.window_rounding = egui::Rounding::same(12.0);
    v.menu_rounding = egui::Rounding::same(8.0);

    style.spacing.button_padding = egui::Vec2::new(18.0, 10.0);
    style.spacing.item_spacing = egui::Vec2::new(10.0, 8.0);
    style.spacing.interact_size = egui::Vec2::new(40.0, 34.0);

    // Text styles
    style.text_styles = [
        (
            egui::TextStyle::Heading,
            egui::FontId::new(26.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            egui::FontId::new(12.5, egui::FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_style(style);
}

struct UnstuckApp {
    rx: Receiver<String>,
    done_rx: Receiver<()>,
    log: Vec<String>,
    busy: bool,
    last_action: Option<Action>,
}

impl UnstuckApp {
    fn new() -> Self {
        Self {
            rx: channel().1,
            done_rx: channel().1,
            log: Vec::new(),
            busy: false,
            last_action: None,
        }
    }

    fn start_action(&mut self, action: Action) {
        let (tx, rx) = channel();
        let (done_tx, done_rx) = channel();
        self.rx = rx;
        self.done_rx = done_rx;
        self.busy = true;
        self.last_action = Some(action);
        self.log.clear();
        thread::spawn(move || {
            run_action(action, |msg| {
                let _ = tx.send(msg.to_string());
            });
            let _ = done_tx.send(());
        });
    }
}

impl eframe::App for UnstuckApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain log messages from the worker thread.
        while let Ok(msg) = self.rx.try_recv() {
            self.log.push(msg);
        }
        // Check whether the worker has finished.
        if self.busy {
            match self.done_rx.try_recv() {
                Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    while let Ok(msg) = self.rx.try_recv() {
                        self.log.push(msg);
                    }
                    self.busy = false;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        let busy = self.busy;
        let log: Vec<String> = self.log.clone();

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SPOTIFY_BLACK))
            .show(ctx, |ui| {
                let mut step1 = false;
                let mut step2 = false;

                // ── Header ────────────────────────────────────────────────
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    // Spotify-style green circle logo
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.circle_filled(rect.center(), 22.0, SPOTIFY_GREEN);
                    // Three "sound wave" arcs
                    let c = rect.center();
                    for (i, (len, w)) in [(10.0, 3.0_f32), (15.0, 3.5_f32), (20.0, 4.0_f32)].iter().enumerate() {
                        let x = c.x - 8.0 + i as f32 * 8.0;
                        painter.line_segment(
                            [
                                egui::pos2(x, c.y + len * 0.5),
                                egui::pos2(x, c.y - len * 0.5),
                            ],
                            egui::Stroke::new(*w, SPOTIFY_BLACK),
                        );
                    }
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.heading(
                            egui::RichText::new("Unstuck Spotify")
                                .color(TEXT_PRIMARY)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("Fix a hung or crashed Spotify app")
                                .color(TEXT_SECONDARY)
                                .size(13.0),
                        );
                    });
                });
                ui.add_space(14.0);

                // ── Action cards ──────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.set_min_width(ui.available_width() - 20.0);

                        // Step 1 card
                        egui::Frame::none()
                            .fill(SPOTIFY_CARD)
                            .rounding(egui::Rounding::same(12.0))
                            .inner_margin(egui::Margin::same(16.0))
                            .stroke(egui::Stroke::new(1.0_f32, SPOTIFY_GREEN_DIM))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(
                                    egui::RichText::new("STEP 1 · CLEAR CACHE")
                                        .color(SPOTIFY_GREEN)
                                        .size(11.0)
                                        .strong(),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new("Safe fix")
                                        .color(TEXT_PRIMARY)
                                        .size(17.0)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "Quits Spotify, deletes its cache. Keeps your login and playlists. Try this first.",
                                    )
                                    .color(TEXT_SECONDARY)
                                    .size(12.5),
                                );
                                ui.add_space(8.0);
                                ui.add_enabled_ui(!busy, |ui| {
                                    let btn = egui::Button::new(
                                        egui::RichText::new("▶  Clear cache").color(SPOTIFY_BLACK),
                                    )
                                    .fill(SPOTIFY_GREEN)
                                    .rounding(egui::Rounding::same(22.0))
                                    .min_size(egui::vec2(ui.available_width(), 34.0));
                                    if ui.add(btn).clicked() {
                                        step1 = true;
                                    }
                                });
                            });

                        ui.add_space(10.0);

                        // Step 2 card
                        egui::Frame::none()
                            .fill(SPOTIFY_CARD)
                            .rounding(egui::Rounding::same(12.0))
                            .inner_margin(egui::Margin::same(16.0))
                            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0x3A, 0x3A, 0x3A)))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(
                                    egui::RichText::new("STEP 2 · FULL RESET")
                                        .color(TEXT_SECONDARY)
                                        .size(11.0)
                                        .strong(),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new("The nuclear option")
                                        .color(TEXT_PRIMARY)
                                        .size(17.0)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "Removes app data, preferences and caches. You'll be logged out — playlists re-sync from the cloud.",
                                    )
                                    .color(TEXT_SECONDARY)
                                    .size(12.5),
                                );
                                ui.add_space(8.0);
                                ui.add_enabled_ui(!busy, |ui| {
                                    let btn = egui::Button::new(
                                        egui::RichText::new("⟳  Full reset").color(TEXT_PRIMARY),
                                    )
                                    .fill(egui::Color32::from_rgb(0x33, 0x33, 0x33))
                                    .rounding(egui::Rounding::same(22.0))
                                    .min_size(egui::vec2(ui.available_width(), 34.0));
                                    if ui.add(btn).clicked() {
                                        step2 = true;
                                    }
                                });
                            });
                    });
                });

                // ── Log panel ─────────────────────────────────────────────
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new("ACTIVITY")
                            .color(TEXT_SECONDARY)
                            .size(11.0)
                            .strong(),
                    );
                    if busy {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new("● working…")
                                    .color(SPOTIFY_GREEN)
                                    .size(12.0),
                            );
                        });
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    egui::Frame::none()
                        .fill(SPOTIFY_PANEL)
                        .rounding(egui::Rounding::same(10.0))
                        .inner_margin(egui::Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width() - 20.0);
                            ui.set_min_height(ui.available_height().min(180.0));
                            egui::ScrollArea::vertical()
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    if log.is_empty() {
                                        ui.label(
                                            egui::RichText::new("Nothing yet — pick a step above.")
                                                .color(TEXT_SECONDARY)
                                                .size(12.0),
                                        );
                                    } else {
                                        for line in &log {
                                            let color = if line.starts_with("  ") {
                                                TEXT_SECONDARY
                                            } else if line.contains("done") {
                                                SPOTIFY_GREEN
                                            } else {
                                                TEXT_PRIMARY
                                            };
                                            ui.label(
                                                egui::RichText::new(line)
                                                    .monospace()
                                                    .color(color)
                                                    .size(12.5),
                                            );
                                        }
                                    }
                                });
                        });
                });

                // ── Footer ────────────────────────────────────────────────
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(
                            "Playlists live in Spotify's cloud — they are never lost.",
                        )
                        .color(TEXT_SECONDARY)
                        .size(11.5),
                    );
                });

                if step1 {
                    self.start_action(Action::Step1);
                }
                if step2 {
                    self.start_action(Action::Step2);
                }
            });
    }
}

fn run_action<F: Fn(&str)>(action: Action, log: F) {
    kill_spotify(&log);
    match action {
        Action::Step1 => {
            log("Step 1 — clearing cache (safe, keeps login & playlists):");
            for (name, path) in step1_targets() {
                if !exists_anywhere(&path) {
                    log(&format!("  - {} not found: {}", name, path.display()));
                    continue;
                }
                let size = total_size(&path);
                log(&format!(
                    "  - {} — {} ({} bytes)",
                    name,
                    human_size(size),
                    size
                ));
                clear_contents(&path, &log);
            }
            log("Step 1 done.");
            log("Now open Spotify and test it.");
            log("First launch will be slower while the cache rebuilds.");
            log("If it still doesn't work, run Step 2 (full reset).");
        }
        Action::Step2 => {
            log("Step 2 — full reset (removes login & local data; playlists re-sync from the cloud):");
            for (name, path) in step2_targets() {
                if !exists_anywhere(&path) {
                    log(&format!("  - {} not found: {}", name, path.display()));
                    continue;
                }
                let size = total_size(&path);
                log(&format!(
                    "  - {} — {} ({} bytes)",
                    name,
                    human_size(size),
                    size
                ));
                remove_node(&path, &log);
            }
            log("Step 2 done.");
            log("Now open Spotify and log in again.");
            log("Playlists re-sync automatically from the cloud.");
            log("If it still doesn't work, re-download the app from https://www.spotify.com/download.");
        }
    }
}

fn kill_spotify<F: Fn(&str)>(log: &F) {
    log("Quitting Spotify if running...");
    if !is_spotify_running() {
        log("  Spotify is not running.");
        return;
    }
    // Graceful quit first.
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"Spotify\" to quit")
            .output();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/IM", "Spotify.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("pkill").args(["-x", "spotify"]).output();
    }
    for _ in 0..10 {
        if !is_spotify_running() {
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
    if is_spotify_running() {
        log("  Spotify didn't quit cleanly — force killing...");
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("taskkill")
                .args(["/IM", "Spotify.exe", "/F"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("pkill")
                .args(["-9", "-f", "Spotify.app/Contents/MacOS/Spotify"])
                .output();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("pkill").args(["-9", "-x", "spotify"]).output();
        }
        thread::sleep(Duration::from_millis(500));
    }
    log("  Spotify quit.");
}

fn is_spotify_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Spotify.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("Spotify.exe"))
            .unwrap_or(false)
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("pgrep")
            .args(["-f", "Spotify.app/Contents/MacOS/Spotify"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("pgrep")
            .args(["-x", "spotify"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Step 1 — cache locations (safe to delete; keeps login & playlists).
fn step1_targets() -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = Vec::new();
    #[cfg(target_os = "macos")]
    if let Some(h) = home_dir() {
        v.push((
            "Spotify cache".to_string(),
            h.join("Library/Caches/com.spotify.client"),
        ));
        v.push((
            "Spotify launcher cache".to_string(),
            h.join("Library/Caches/com.spotify.client-launcher"),
        ));
    }
    #[cfg(target_os = "windows")]
    if let Some(la) = localappdata() {
        let sp = la.join("Spotify");
        v.push(("Spotify media cache".to_string(), sp.join("Data")));
        v.push(("Spotify browser cache".to_string(), sp.join("Browser")));
    }
    #[cfg(target_os = "linux")]
    if let Some(h) = home_dir() {
        v.push(("Spotify cache".to_string(), h.join(".cache/spotify")));
    }
    v
}

/// Step 2 — full reset locations (logs you out; playlists re-sync from the cloud).
fn step2_targets() -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = Vec::new();
    #[cfg(target_os = "macos")]
    if let Some(h) = home_dir() {
        v.push((
            "Spotify app data".to_string(),
            h.join("Library/Application Support/Spotify"),
        ));
        v.push((
            "Spotify prefs".to_string(),
            h.join("Library/Preferences/com.spotify.client.plist"),
        ));
        v.push((
            "Spotify cache".to_string(),
            h.join("Library/Caches/com.spotify.client"),
        ));
        v.push((
            "Spotify launcher cache".to_string(),
            h.join("Library/Caches/com.spotify.client-launcher"),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(ad) = appdata() {
            v.push(("Spotify app data".to_string(), ad.join("Spotify")));
        }
        if let Some(la) = localappdata() {
            v.push(("Spotify cache".to_string(), la.join("Spotify")));
        }
    }
    #[cfg(target_os = "linux")]
    if let Some(h) = home_dir() {
        v.push(("Spotify app data".to_string(), h.join(".config/spotify")));
        v.push(("Spotify cache".to_string(), h.join(".cache/spotify")));
    }
    v
}

fn exists_anywhere(path: &Path) -> bool {
    fs::symlink_metadata(path).map(|_| true).unwrap_or(false)
}

fn total_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += total_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn clear_contents<F: Fn(&str)>(dir: &Path, log: &F) {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let p = entry.path();
                let _ = fs::symlink_metadata(&p).and_then(|md| {
                    if md.is_dir() {
                        fs::remove_dir_all(&p)
                    } else {
                        fs::remove_file(&p)
                    }
                });
            }
            log(&format!("  cleared {}", dir.display()));
        }
        Err(e) => log(&format!("  (could not read {}: {})", dir.display(), e)),
    }
}

fn remove_node<F: Fn(&str)>(path: &Path, log: &F) {
    match fs::symlink_metadata(path).and_then(|md| {
        if md.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }) {
        Ok(()) => log(&format!("  removed {}", path.display())),
        Err(e) => log(&format!("  (could not remove {}: {})", path.display(), e)),
    }
}

#[cfg(target_os = "windows")]
fn appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "windows")]
fn localappdata() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

// Linux uses HOME for both cache and config; macOS uses HOME too.
#[cfg(target_os = "linux")]
fn localappdata() -> Option<PathBuf> {
    home_dir()
}
