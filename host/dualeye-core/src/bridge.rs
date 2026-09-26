//! Sample sensors at a fixed rate and write each snapshot to the board,
//! reconnecting when the USB cable or the board goes away.
//!
//! Frontends observe the loop through [`BridgeEvent`]s: the CLI prints them,
//! a Tauri app can forward them to the webview with `app.emit(..)`.

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::sensors::Collector;
use crate::serial;
use crate::snapshot::Snapshot;

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Serial port; `None` auto-detects the Espressif board on every (re)connect.
    pub port: Option<String>,
    /// Time between snapshots. The firmware marks data stale after 3 s.
    pub interval: Duration,
    /// Pause after opening the port, in case opening it rebooted the board.
    pub boot_wait: Duration,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self { port: None, interval: Duration::from_secs(1), boot_wait: Duration::from_secs(2) }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeEvent {
    /// No board to talk to yet; the bridge keeps retrying.
    Waiting { reason: String },
    Connected { port: String },
    /// A snapshot was taken; `sent` is false when it had no temperature to show.
    Snapshot { snapshot: Snapshot, sent: bool },
    /// A line the firmware logged on the same USB port.
    BoardLog { line: String },
    Disconnected { port: String, reason: String, permission_denied: bool },
}

pub type EventSink = Arc<dyn Fn(BridgeEvent) + Send + Sync>;

/// The bridge loop on its own thread. Dropping it stops the loop.
pub struct Bridge {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Bridge {
    pub fn spawn(config: BridgeConfig, on_event: impl Fn(BridgeEvent) + Send + Sync + 'static) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let sink: EventSink = Arc::new(on_event);
        let thread = thread::Builder::new()
            .name("dualeye-bridge".into())
            .spawn(move || run(&config, &flag, sink))
            .expect("spawn bridge thread");
        Self { stop, thread: Some(thread) }
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Run the bridge on the current thread until `stop` is set.
pub fn run(config: &BridgeConfig, stop: &AtomicBool, on_event: EventSink) {
    let mut collector = Collector::new();
    let mut delay = Duration::from_secs(1);
    while !stop.load(Ordering::Relaxed) {
        let Some(port) = config.port.clone().or_else(serial::detect_board) else {
            on_event(BridgeEvent::Waiting { reason: "board not found on USB".into() });
            sleep_unless_stopped(delay, stop);
            delay = (delay * 2).min(Duration::from_secs(5));
            continue;
        };
        let started = Instant::now();
        if let Err(err) = session(&port, config, stop, &mut collector, &on_event) {
            on_event(BridgeEvent::Disconnected {
                port,
                reason: err.to_string(),
                permission_denied: err.kind() == io::ErrorKind::PermissionDenied,
            });
            if started.elapsed() > Duration::from_secs(10) {
                delay = Duration::from_secs(1);
            }
            sleep_unless_stopped(delay, stop);
            delay = (delay * 2).min(Duration::from_secs(15));
        }
    }
}

fn session(
    port: &str,
    config: &BridgeConfig,
    stop: &AtomicBool,
    collector: &mut Collector,
    on_event: &EventSink,
) -> io::Result<()> {
    let mut ser = serial::open(port)?;
    on_event(BridgeEvent::Connected { port: port.to_string() });

    let reader_stop = Arc::new(AtomicBool::new(false));
    let reader = {
        let rx = ser.try_clone()?;
        let flag = reader_stop.clone();
        let sink = on_event.clone();
        thread::spawn(move || forward_board_log(rx, &flag, &sink))
    };

    let result = (|| {
        sleep_unless_stopped(config.boot_wait, stop);
        let mut next = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            let snapshot = collector.sample();
            let sent = snapshot.is_sendable();
            if sent {
                ser.write_all(snapshot.to_line().as_bytes())?;
                ser.flush()?;
            }
            on_event(BridgeEvent::Snapshot { snapshot, sent });
            next += config.interval;
            let now = Instant::now();
            if next < now {
                next = now;
            }
            sleep_unless_stopped(next - now, stop);
        }
        Ok(())
    })();

    reader_stop.store(true, Ordering::Relaxed);
    let _ = reader.join();
    result
}

fn forward_board_log(mut rx: Box<dyn serialport::SerialPort>, stop: &AtomicBool, sink: &EventSink) {
    let mut buf = [0u8; 256];
    let mut line = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        match rx.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => {
                for &b in &buf[..n] {
                    match b {
                        b'\n' => {
                            let text = String::from_utf8_lossy(&line).trim_end_matches('\r').to_string();
                            line.clear();
                            if !text.is_empty() {
                                sink(BridgeEvent::BoardLog { line: text });
                            }
                        }
                        _ if line.len() < 1024 => line.push(b),
                        _ => {}
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {}
            Err(_) => return,
        }
    }
}

fn sleep_unless_stopped(duration: Duration, stop: &AtomicBool) {
    let end = Instant::now() + duration;
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now >= end {
            return;
        }
        thread::sleep((end - now).min(Duration::from_millis(50)));
    }
}
