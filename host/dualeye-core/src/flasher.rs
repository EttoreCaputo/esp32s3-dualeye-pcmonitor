//! Identify and flash the board through Espressif's `esptool`
//! (<https://docs.espressif.com/projects/esptool/>), run as a child process.
//!
//! esptool lives in a private virtualenv under a directory the caller owns
//! (the app's data folder); with the `provision` feature, [`setup::ensure`]
//! creates it on first use, downloading Python if the machine has none, so the
//! user installs nothing. The serial port must be free (stop the
//! [`Bridge`](crate::Bridge) first): esptool resets the chip into its ROM
//! bootloader and back.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use serde::Serialize;

/// Where the merged image goes: it carries bootloader, partition table and app.
pub const MERGED_OFFSET: &str = "0x0";
const CHIP: &str = "esp32s3";
const FLASH_BAUD: &str = "460800";
/// Written into the virtualenv once esptool is installed in it.
pub(crate) const READY_MARKER: &str = ".dualeye-ready";

#[cfg(feature = "provision")]
pub mod setup;

#[derive(Debug, Clone, Serialize)]
pub struct Esptool {
    /// The virtualenv's interpreter, run as `python -m esptool`.
    pub python: PathBuf,
    pub version: String,
}

/// What esptool reports about the chip on the other end of the port.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChipInfo {
    pub port: String,
    pub chip: Option<String>,
    pub features: Option<String>,
    pub crystal: Option<String>,
    pub mac: Option<String>,
    pub flash_size: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlashEvent {
    /// A line of esptool output.
    Log { line: String },
    /// Share of the image written so far, 0–100.
    Progress { percent: f32 },
    /// Preparing esptool itself (first use only); `percent` while downloading.
    Setup { message: String, percent: Option<f32> },
}

impl Esptool {
    /// esptool from the virtualenv in `dir`, if it has been set up and runs.
    pub fn installed(dir: &Path) -> Option<Self> {
        let venv = dir.join("venv");
        if !venv.join(READY_MARKER).is_file() {
            return None;
        }
        let python = venv_python(&venv);
        let out = command(&python).args(["-m", "esptool", "version"]).stdin(Stdio::null()).output().ok()?;
        if !out.status.success() {
            return None;
        }
        // `version` prints a banner then the bare version number.
        let text = String::from_utf8_lossy(&out.stdout);
        let version = text.lines().map(str::trim).filter(|l| !l.is_empty()).last()?.to_string();
        Some(Self { python, version })
    }

    /// Connect to the chip on `port` and read its identity and flash size.
    pub fn chip_info(&self, port: &str, on_event: impl FnMut(FlashEvent)) -> io::Result<ChipInfo> {
        let lines = self.run(&["--port", port, "flash-id"], on_event)?;
        let mut info = ChipInfo { port: port.to_string(), ..Default::default() };
        for line in &lines {
            let value = |keys: &[&str]| keys.iter().find_map(|k| line.strip_prefix(k)).map(|v| v.trim().to_string());
            if let Some(v) = value(&["Chip type:"]) {
                info.chip = Some(v);
            } else if let Some(v) = value(&["Features:"]) {
                info.features = Some(v);
            } else if let Some(v) = value(&["Crystal frequency:"]) {
                info.crystal = Some(v);
            } else if let Some(v) = value(&["MAC:"]) {
                info.mac.get_or_insert(v);
            } else if let Some(v) = value(&["Detected flash size:"]) {
                info.flash_size = Some(v);
            }
        }
        Ok(info)
    }

    /// Write a merged image (bootloader + partitions + app) at 0x0, verify it
    /// and reset the board into the new firmware.
    pub fn flash(&self, port: &str, image: &Path, on_event: impl FnMut(FlashEvent)) -> io::Result<()> {
        let image = image.to_string_lossy();
        let args = ["--chip", CHIP, "--port", port, "--baud", FLASH_BAUD, "write-flash", MERGED_OFFSET, &image];
        self.run(&args, on_event).map(drop)
    }

    /// Run esptool, forwarding its output line by line; returns every line.
    fn run(&self, args: &[&str], on_event: impl FnMut(FlashEvent)) -> io::Result<Vec<String>> {
        let mut cmd = command(&self.python);
        cmd.args(["-m", "esptool"]).args(args);
        run_streaming(cmd, on_event)
    }
}

/// Run a Python process, forwarding its output (progress bars included) line
/// by line; returns every non-progress line.
pub(crate) fn run_streaming(mut cmd: Command, mut on_event: impl FnMut(FlashEvent)) -> io::Result<Vec<String>> {
    let mut child = cmd
        .env("PYTHONUNBUFFERED", "1")
        // The progress bar is drawn with box characters; a legacy Windows code page can't encode them.
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>);
    let stderr = child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>);
    let readers = [stdout, stderr]
        .into_iter()
        .flatten()
        .map(|pipe| {
            let tx = tx.clone();
            thread::spawn(move || split_lines(pipe, |line| drop(tx.send(line))))
        })
        .collect::<Vec<_>>();
    drop(tx);

    let mut lines = Vec::new();
    let mut fatal = None;
    let mut permission_denied = false;
    for line in rx {
        if let Some(percent) = progress(&line) {
            on_event(FlashEvent::Progress { percent });
            continue;
        }
        // "ERROR: A fatal error occurred: Could not open /dev/ttyACM0, the port is busy or doesn't exist."
        if let Some(reason) = line.split_once("A fatal error occurred:").map(|(_, r)| r.trim()) {
            fatal.get_or_insert_with(|| reason.to_string());
        } else if fatal.is_none() && line.starts_with("ERROR:") {
            fatal = Some(line.trim_start_matches("ERROR:").trim().to_string());
        }
        permission_denied |= line.contains("Permission denied");
        on_event(FlashEvent::Log { line: line.clone() });
        lines.push(line);
    }
    for reader in readers {
        let _ = reader.join();
    }

    let status = child.wait()?;
    if status.success() {
        Ok(lines)
    } else {
        let reason = fatal.unwrap_or_else(|| format!("{:?} exited with {status}", cmd.get_program()));
        let kind = if permission_denied { io::ErrorKind::PermissionDenied } else { io::ErrorKind::Other };
        Err(io::Error::new(kind, reason))
    }
}

pub(crate) fn venv_python(venv: &Path) -> PathBuf {
    if cfg!(windows) { venv.join("Scripts").join("python.exe") } else { venv.join("bin").join("python") }
}

pub(crate) fn command(program: &Path) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        // Don't flash a console window from a GUI app.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Split a stream on `\n` and `\r` (esptool redraws its progress with `\r`).
fn split_lines(mut pipe: impl Read, mut emit: impl FnMut(String)) {
    let mut buf = [0u8; 512];
    let mut line = Vec::new();
    loop {
        match pipe.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                for &b in &buf[..n] {
                    if b == b'\n' || b == b'\r' {
                        let text = String::from_utf8_lossy(&line).trim().to_string();
                        line.clear();
                        if !text.is_empty() {
                            emit(text);
                        }
                    } else {
                        line.push(b);
                    }
                }
            }
        }
    }
    let text = String::from_utf8_lossy(&line).trim().to_string();
    if !text.is_empty() {
        emit(text);
    }
}

/// `Writing at 0x00010000 ━━━━━━      35.2% 192.00kB/546.25kB [1s]` (v4 printed `... (12 %)`).
fn progress(line: &str) -> Option<f32> {
    if !line.starts_with("Writing at") {
        return None;
    }
    let head = &line[..line.find('%')?];
    let start = head.trim_end().rfind(|c: char| !(c.is_ascii_digit() || c == '.')).map_or(0, |i| i + 1);
    head.trim_end()[start..].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_from_both_major_versions() {
        assert_eq!(progress("Writing at 0x00010000... (12 %)"), Some(12.0));
        assert_eq!(progress("Writing at 0x00014e20 ━━━━━━━━━                       30.0% 29.30kB/97.66kB [0s]"), Some(30.0));
        assert_eq!(progress("Wrote 559360 bytes (100 %)"), None);
        assert_eq!(progress("Writing at 0x0"), None);
    }

    #[test]
    fn splits_on_carriage_returns() {
        let mut lines = Vec::new();
        split_lines(&b"Connecting...\nWriting (1 %)\rWriting (2 %)\r\nDone"[..], |l| lines.push(l));
        assert_eq!(lines, ["Connecting...", "Writing (1 %)", "Writing (2 %)", "Done"]);
    }
}
