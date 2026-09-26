# ESP32-S3 DualEye PC Monitor

![Target](https://img.shields.io/badge/target-ESP32--S3-brightgreen)
![ESP-IDF](https://img.shields.io/badge/ESP--IDF-%3E%3D5.4-blue)
![LVGL](https://img.shields.io/badge/LVGL-9.3.0-orange)
![LCD](https://img.shields.io/badge/LCD-GC9A01%20×2-lightgrey)
![Host](https://img.shields.io/badge/host-Rust-purple)

PC monitoring on **ESP32-S3 DualEye** (two 240×240 round displays): CPU/GPU temperatures and fans, Apple Watch–style UI.

A standalone Rust program on the host reads the sensors straight from the OS (no CoolerControl or other daemon) and sends them to the board over USB Serial/JTAG as one JSON line per second.

| Piece | Role |
|-------|------|
| Firmware (`main/`) | LVGL UI + metrics ingest |
| `host/dualeye-core` | Library: sensors → snapshot → USB serial. Meant to be embedded in a [Tauri](https://v2.tauri.app/) app |
| `host/dualeye-cli` | `dualeye` command-line bridge built on the library |
| `host/dualeye-app` | Desktop app ([Tauri 2](https://v2.tauri.app/) + Svelte): live mirror of both screens, history, sensors, board console, firmware flashing |

End users only need the desktop app installer; everything below the app section is for working on the source (see [Building from source](#building-from-source)).

**Stack:** ESP-IDF ≥ 5.4 · [lvgl/lvgl](https://components.espressif.com/components/lvgl/lvgl) `9.3.0` · [espressif/esp_lcd_gc9a01](https://components.espressif.com/components/espressif/esp_lcd_gc9a01) `^2.0.4` · Rust (sysinfo, nvml-wrapper, serialport)

## Host bridge

Install Rust from [rustup.rs](https://rustup.rs), then close `idf.py monitor` (it holds the same port) and run:

```bash
cd host
cargo run --release              # auto-detects the board (USB 303a:xxxx) and streams
cargo run --release -- --once    # print one snapshot, no serial
cargo run --release -- --sensors # every raw sensor the backends can see
cargo run --release -- --help
```

The binary ends up in `host/target/release/dualeye` (`dualeye.exe` on Windows) and has no runtime dependencies.

### What each OS provides

| Data | Linux | Windows | macOS |
|------|-------|---------|-------|
| CPU load, clock | ✓ | ✓ | ✓ |
| CPU temp (avg of all CPU sensors) | hwmon: coretemp, k10temp, zenpower | ACPI thermal zone (run as admin; many boards report nothing) | SMC / IOHID |
| CPU power | RAPL (see below) | — | — |
| NVIDIA GPU (temp, load, clock, power, fan RPM) | NVML | NVML | — |
| AMD GPU | hwmon `amdgpu` | — | — |
| Apple GPU temp | — | — | IOHID |
| Fans | hwmon (`cpu` = fastest board fan, `gpu` = fastest GPU fan) | — | — |

Missing values are just left out of the snapshot; the board shows what it gets.

**Linux, serial access:** `sudo usermod -aG dialout "$USER"`, then log out and back in.

**Linux, CPU power:** `energy_uj` is root-only since the RAPL side-channel fix (CVE-2020-8694). To show package power anyway, make it world-readable at boot:

```bash
echo 'z /sys/class/powercap/intel-rapl:0/energy_uj 0444 - - -' | sudo tee /etc/tmpfiles.d/dualeye-rapl.conf
```

```bash
sudo systemd-tmpfiles --create /etc/tmpfiles.d/dualeye-rapl.conf
```

## Desktop app

`host/dualeye-app` runs the same bridge in the background and shows, in real time, what the two round screens are displaying. The mirror follows `main/ui_watch.c` (ring geometry, fonts, colours, warm/hot/stale/waiting states), so keep them in sync when the firmware UI changes. It also plots 3 minutes of history and lists raw sensors, serial ports and the board's log. Closing the window hides it in the tray and streaming continues.

How to run and package it is in [Building from source](#building-from-source).

### Identify and flash the board

Settings → **Device** finds the ESP32-S3 on USB, reads its chip, MAC and flash size, and flashes the firmware. The image is `build/merged-binary.bin` (bootloader + partition table + app, written at `0x0`), embedded in the app at build time, so replace that file and rebuild the app to ship a new firmware. It works on a blank board too, since esptool talks to the ROM bootloader; streaming pauses meanwhile and the board reboots afterwards.

Flashing uses [esptool](https://docs.espressif.com/projects/esptool/en/latest/esp32s3/index.html) and the user installs nothing: the first time Identify or Flash is used, the app

1. takes the system Python if it is 3.10+ with `venv` (Linux, Windows), otherwise downloads a portable CPython from [python-build-standalone](https://github.com/astral-sh/python-build-standalone), pinned and SHA-256 checked (never the system one on macOS, whose stub opens the Xcode installer);
2. creates a virtualenv and `pip install`s esptool 5.x into it.

That is about 45 MB downloaded once (internet needed only then) and ~170 MB on disk, in the app's local data folder under `esptool/` (`~/.local/share/com.dualeye.monitor/esptool` on Linux, `%LOCALAPPDATA%\com.dualeye.monitor\esptool` on Windows, `~/Library/Application Support/com.dualeye.monitor/esptool` on macOS). The output of the last setup is in `setup.log` there, and the Device tab shows it too when something fails. Delete that folder to force a fresh setup. The logic is in `dualeye-core` behind the `provision` feature, which only the app enables. The equivalent command line is:

```bash
esptool --chip esp32s3 --port /dev/ttyACM0 write-flash 0x0 build/merged-binary.bin
```

The app talks to the bridge through `dualeye_core::Bridge`; its events (`waiting`, `connected`, `snapshot`, `board_log`, `disconnected`) are forwarded to the webview as `bridge`.

## Building from source

### Repository layout

```
main/                     firmware (ESP-IDF component: display, LVGL UI, metrics parser)
build/merged-binary.bin   firmware image the desktop app flashes (the only tracked file in build/)
sdkconfig.defaults        firmware config (target esp32s3, 16 MB flash, USB Serial/JTAG console)
.devcontainer/            ESP-IDF container for VS Code
host/                     Cargo workspace
  dualeye-core/           sensors, snapshot, serial bridge, esptool setup/flash (library)
  dualeye-cli/            `dualeye` command-line bridge
  dualeye-app/            desktop app: Svelte UI in src/, Tauri shell in src-tauri/
```

### Prerequisites

| Part | Needs |
|------|-------|
| Firmware | [ESP-IDF](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/get-started/) ≥ 5.4, or the dev container in `.devcontainer/` |
| CLI | [Rust](https://rustup.rs) ≥ 1.85 (edition 2024) |
| Desktop app | Rust, [Node.js](https://nodejs.org) ≥ 20.19, plus the platform packages below |

Desktop app, per platform (once):

- **Linux (Debian/Ubuntu):**

  ```bash
  sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev build-essential
  ```

  Other distros: see [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/). Add yourself to `dialout` for serial access (see above).
- **Windows:** [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++" and the Rust MSVC toolchain. WebView2 ships with Windows 10/11.
- **macOS:** `xcode-select --install`.

### Firmware

```bash
idf.py set-target esp32s3   # first time only
idf.py build
idf.py -p /dev/ttyACM0 flash monitor
```

To update the image the desktop app ships, merge bootloader, partition table and app into one file and commit it:

```bash
idf.py merge-bin            # writes build/merged-binary.bin
```

`.gitignore` ignores `build/` except that file. The app embeds it at compile time (`include_bytes!` in `host/dualeye-app/src-tauri/src/lib.rs`), so rebuild the app afterwards.

### CLI

```bash
cd host
cargo build --release       # host/target/release/dualeye
```

Plain `cargo` commands in `host/` only touch `dualeye-core` and `dualeye-cli` (the workspace's default members), so they work without the GUI packages.

### Desktop app

```bash
cd host/dualeye-app
npm install
npm run dev                 # UI only, in a browser, with synthetic data: no board or Rust needed
npm run tauri dev           # the real app, hot reload on UI changes
npm run tauri build         # release build + installers
```

`npm run tauri build` packages for the OS it runs on (Tauri does not cross-compile), into `host/target/release/bundle/`:

| Built on | Output |
|----------|--------|
| Linux | `deb/*.deb`, `rpm/*.rpm`, `appimage/*.AppImage` |
| Windows | `msi/*.msi`, `nsis/*-setup.exe` |
| macOS | `macos/*.app`, `dmg/*.dmg` (`-- --target universal-apple-darwin` for Intel + Apple Silicon, after `rustup target add aarch64-apple-darwin x86_64-apple-darwin`) |

To ship all three from one place, run the build on a CI matrix (e.g. GitHub Actions with [tauri-action](https://github.com/tauri-apps/tauri-action)). Unsigned builds work but Windows SmartScreen and macOS Gatekeeper warn about them; signing needs a code-signing certificate / Apple Developer ID.

Where things live in the app:

| What | Where |
|------|-------|
| Screen mirror (keep in sync with `main/ui_watch.c`) | `src/lib/Board.svelte`, `Eye.svelte`, `firmware.ts` |
| Live state, bridge/flash events, synthetic preview feed | `src/lib/monitor.svelte.ts` |
| Settings drawer (Connection, Device, Sensors, Console) | `src/lib/Drawer.svelte` |
| Tauri commands, tray, bridge lifecycle | `src-tauri/src/lib.rs` |
| esptool runner and first-use setup | `host/dualeye-core/src/flasher.rs`, `flasher/setup.rs` |

### Checks

```bash
cd host && cargo test -p dualeye-core --features provision
```

```bash
cd host/dualeye-app && npm run check
```

## Wire format

```json
{"v":1,"ts":1790358954,"cpu":{"temp_c":38.4,"load_pct":3.3,"clock_mhz":1187,"power_w":14.6},"gpu":{"temp_c":34.0,"load_pct":0.0,"clock_mhz":210,"power_w":21.2},"fans":[{"id":"cpu","rpm":3813},{"id":"gpu","rpm":0}]}
```

Parsed by `main/metrics_parser.c`; lines without any temperature are ignored, and the UI goes stale after 3 s without data.
