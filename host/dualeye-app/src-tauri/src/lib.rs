//! Tauri shell around `dualeye-core`: the bridge runs on its own thread for the
//! whole life of the process, and every event it raises is kept here (so a
//! freshly loaded webview can catch up) and forwarded to the UI as `bridge`.
//!
//! Closing the window only hides it; the tray icon brings it back or quits.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use dualeye_core::{Bridge, BridgeConfig, BridgeEvent, Collector, PortInfo, Reading, Snapshot, serial};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

const LOG_LINES: usize = 300;
const TRAY_ID: &str = "main";

#[derive(Default, Clone, Serialize, Deserialize)]
struct Settings {
    /// `None` auto-detects the board.
    port: Option<String>,
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
    /// Separate from the bridge's own collector, for the sensor list.
    collector: Mutex<Option<Collector>>,
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

fn start_bridge(app: &AppHandle, port: Option<String>) -> Bridge {
    let handle = app.clone();
    Bridge::spawn(BridgeConfig { port, ..Default::default() }, move |event| {
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
            let settings: Settings = settings_path
                .as_ref()
                .and_then(|p| fs::read_to_string(p).ok())
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let port = settings.port.clone();
            app.manage(AppState {
                link: Mutex::new(Link { kind: "searching", ..Default::default() }),
                bridge: Mutex::new(None),
                settings: Mutex::new(settings),
                settings_path,
                collector: Mutex::new(None),
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
        .invoke_handler(tauri::generate_handler![status, list_ports, set_port, readings])
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
