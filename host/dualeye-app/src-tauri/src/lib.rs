//! Tauri shell around `dualeye-core`: the bridge runs on its own thread for the
//! whole life of the process, and every event it raises is kept here (so a
//! freshly loaded webview can catch up) and forwarded to the UI as `bridge`.
//!
//! Closing the window only hides it; the tray icon brings it back or quits.
//!
//! Identifying and flashing the board go through esptool, which the app sets
//! up by itself on first use (Python + virtualenv in its data folder); the
//! bridge is stopped meanwhile so esptool can own the port, then started again.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use dualeye_core::flasher::setup;
use dualeye_core::{
    Bridge, BridgeConfig, BridgeEvent, ChipInfo, Collector, Esptool, Faces, FlashEvent, PortInfo, Reading, Snapshot, serial,
};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

const LOG_LINES: usize = 300;
const TRAY_ID: &str = "main";
/// The image in the repository's `build/` folder, shipped inside the app.
const FIRMWARE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../build/merged-binary.bin"));

#[derive(Default, Clone, Serialize, Deserialize)]
struct Settings {
    /// `None` auto-detects the board.
    port: Option<String>,
    #[serde(default)]
    faces: Faces,
}

#[derive(Default)]
struct Link {
    kind: &'static str,
    port: Option<String>,
    message: Option<String>,
    last: Option<Snapshot>,
    sent: Option<Snapshot>,
    sent_at: Option<Instant>,
    connected_at: Option<Instant>,
    logs: VecDeque<String>,
}

impl Link {
    fn record(&mut self, event: &BridgeEvent) {
        match event {
            BridgeEvent::Waiting { reason } => {
                self.kind = "searching";
                self.message = Some(reason.clone());
            }
            BridgeEvent::Connected { port } => {
                self.kind = "connected";
                self.port = Some(port.clone());
                self.message = None;
                self.sent = None;
                self.connected_at = Some(Instant::now());
            }
            BridgeEvent::Snapshot { snapshot, sent } => {
                self.last = Some(snapshot.clone());
                if *sent {
                    self.sent = Some(snapshot.clone());
                    self.sent_at = Some(Instant::now());
                }
            }
            BridgeEvent::BoardLog { line } => {
                if self.logs.len() == LOG_LINES {
                    self.logs.pop_front();
                }
                self.logs.push_back(line.clone());
            }
            BridgeEvent::Disconnected { reason, .. } => {
                self.kind = "offline";
                self.message = Some(reason.clone());
            }
        }
    }
}

struct AppState {
    link: Mutex<Link>,
    bridge: Mutex<Option<Bridge>>,
    settings: Mutex<Settings>,
    settings_path: Option<PathBuf>,
    /// Handed to every bridge, so a face change reaches the running one.
    faces: Arc<Mutex<Faces>>,
    /// Separate from the bridge's own collector, for the sensor list.
    collector: Mutex<Option<Collector>>,
    /// esptool holds the port (identify or flash in progress).
    device_busy: AtomicBool,
    /// Where esptool's Python and virtualenv live.
    esptool_dir: PathBuf,
}

impl AppState {
    fn save_settings(&self) {
        let Some(path) = &self.settings_path else { return };
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let settings = self.settings.lock().unwrap().clone();
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = fs::write(path, json);
        }
    }
}

#[derive(Serialize)]
struct Status {
    link: &'static str,
    port: Option<String>,
    message: Option<String>,
    last: Option<Snapshot>,
    sent: Option<Snapshot>,
    sent_age_ms: Option<u64>,
    connected_age_ms: Option<u64>,
    logs: Vec<String>,
    port_setting: Option<String>,
    faces: Faces,
}

#[tauri::command]
fn status(state: State<AppState>) -> Status {
    let link = state.link.lock().unwrap();
    let age = |t: Option<Instant>| t.map(|t| t.elapsed().as_millis() as u64);
    Status {
        link: link.kind,
        port: link.port.clone(),
        message: link.message.clone(),
        last: link.last.clone(),
        sent: link.sent.clone(),
        sent_age_ms: age(link.sent_at),
        connected_age_ms: age(link.connected_at),
        logs: link.logs.iter().cloned().collect(),
        port_setting: state.settings.lock().unwrap().port.clone(),
        faces: *state.faces.lock().unwrap(),
    }
}

#[tauri::command]
fn list_ports() -> Vec<PortInfo> {
    serial::list_ports()
}

#[tauri::command]
async fn set_port(app: AppHandle, port: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.settings.lock().unwrap().port = port.clone();
        state.save_settings();
        if state.device_busy.load(Ordering::SeqCst) {
            // The bridge comes back with the new setting once esptool is done.
            return;
        }
        // Dropping the old bridge joins its thread and releases the port.
        let old = state.bridge.lock().unwrap().take();
        drop(old);
        *state.link.lock().unwrap() = Link { kind: "searching", ..Default::default() };
        let bridge = start_bridge(&app, port);
        *state.bridge.lock().unwrap() = Some(bridge);
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_faces(state: State<AppState>, faces: Faces) {
    *state.faces.lock().unwrap() = faces;
    state.settings.lock().unwrap().faces = faces;
    state.save_settings();
}

#[tauri::command]
async fn readings(app: AppHandle) -> Result<Vec<Reading>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut slot = state.collector.lock().unwrap();
        let collector = slot.get_or_insert_with(Collector::new);
        collector.readings()
    })
    .await
    .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct FirmwareInfo {
    size: usize,
    /// `None` until esptool has been set up (done on first identify/flash).
    esptool: Option<Esptool>,
}

#[tauri::command]
async fn firmware_info(app: AppHandle) -> Result<FirmwareInfo, String> {
    let dir = app.state::<AppState>().esptool_dir.clone();
    let esptool = tauri::async_runtime::spawn_blocking(move || Esptool::installed(&dir)).await.map_err(|e| e.to_string())?;
    Ok(FirmwareInfo { size: FIRMWARE.len(), esptool })
}

#[tauri::command]
async fn identify_board(app: AppHandle, port: Option<String>) -> Result<ChipInfo, String> {
    with_device(app, port, |app, tool, port| tool.chip_info(port, |e| emit_flash(app, e))).await
}

#[tauri::command]
async fn flash_board(app: AppHandle, port: Option<String>) -> Result<(), String> {
    with_device(app, port, |app, tool, port| {
        let path = std::env::temp_dir().join("dualeye-merged-binary.bin");
        fs::write(&path, FIRMWARE)?;
        let result = tool.flash(port, &path, |e| emit_flash(app, e));
        let _ = fs::remove_file(&path);
        result
    })
    .await
}

fn emit_flash(app: &AppHandle, event: FlashEvent) {
    let _ = app.emit("flash", &event);
}

/// Set esptool up if needed, stop the bridge, hand the port to esptool, then
/// start the bridge again.
async fn with_device<T: Send + 'static>(
    app: AppHandle,
    port: Option<String>,
    job: impl FnOnce(&AppHandle, &Esptool, &str) -> std::io::Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if state.device_busy.swap(true, Ordering::SeqCst) {
            return Err("esptool is already talking to the board".to_string());
        }
        let result = (|| -> Result<T, String> {
            // Streaming carries on while Python and esptool download.
            let tool = setup::ensure(&state.esptool_dir, |e| emit_flash(&app, e)).map_err(|e| format!("setting up esptool: {e}"))?;
            let port = port
                .or_else(|| state.settings.lock().unwrap().port.clone())
                .or_else(serial::detect_board)
                .ok_or("no single DualEye found on USB: pick its port")?;
            let old = state.bridge.lock().unwrap().take();
            drop(old);
            let paused = BridgeEvent::Waiting { reason: "esptool is using the port".into() };
            let mut link = Link { kind: "searching", ..Default::default() };
            link.record(&paused);
            *state.link.lock().unwrap() = link;
            let _ = app.emit("bridge", &paused);
            let result = job(&app, &tool, &port).map_err(|e| e.to_string());
            let bridge = start_bridge(&app, state.settings.lock().unwrap().port.clone());
            *state.bridge.lock().unwrap() = Some(bridge);
            result
        })();
        state.device_busy.store(false, Ordering::SeqCst);
        result
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|result| result)
}

fn start_bridge(app: &AppHandle, port: Option<String>) -> Bridge {
    let handle = app.clone();
    let faces = app.state::<AppState>().faces.clone();
    Bridge::spawn(BridgeConfig { port, faces, ..Default::default() }, move |event| {
        if let Some(state) = handle.try_state::<AppState>() {
            state.link.lock().unwrap().record(&event);
        }
        if let BridgeEvent::Snapshot { snapshot, .. } = &event {
            update_tray(&handle, snapshot);
        }
        let _ = handle.emit("bridge", &event);
    })
}

fn update_tray(app: &AppHandle, s: &Snapshot) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let temp = |t: Option<f32>| t.map_or_else(|| "—".to_string(), |t| format!("{t:.0}°"));
    let text = format!("CPU {} · GPU {}", temp(s.cpu.temp_c), temp(s.gpu.temp_c));
    let _ = tray.set_tooltip(Some(format!("DualEye — {text}")));
}

fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show DualEye", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("DualEye")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let settings_path = app.path().app_config_dir().ok().map(|d| d.join("settings.json"));
            let esptool_dir = app.path().app_local_data_dir()?.join("esptool");
            let settings: Settings = settings_path
                .as_ref()
                .and_then(|p| fs::read_to_string(p).ok())
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let port = settings.port.clone();
            let faces = Arc::new(Mutex::new(settings.faces));
            app.manage(AppState {
                link: Mutex::new(Link { kind: "searching", ..Default::default() }),
                bridge: Mutex::new(None),
                settings: Mutex::new(settings),
                settings_path,
                faces,
                collector: Mutex::new(None),
                device_busy: AtomicBool::new(false),
                esptool_dir,
            });
            let handle = app.handle();
            let bridge = start_bridge(handle, port);
            *app.state::<AppState>().bridge.lock().unwrap() = Some(bridge);
            build_tray(handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Keep streaming to the board when the window is closed.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![status, list_ports, set_port, set_faces, readings, firmware_info, identify_board, flash_board])
        .build(tauri::generate_context!())
        .expect("failed to build the DualEye app")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Release the serial port before the process goes away.
                if let Some(state) = app.try_state::<AppState>() {
                    state.bridge.lock().unwrap().take();
                }
            }
        });
}
