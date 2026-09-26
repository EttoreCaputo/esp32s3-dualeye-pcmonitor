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
| `host/dualeye-app` | Desktop app ([Tauri 2](https://v2.tauri.app/) + Svelte): live mirror of both screens, history, sensors, board console |

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

Linux build dependencies (once):

```bash
sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev build-essential
```

Then:

```bash
cd host/dualeye-app
npm install
npm run tauri dev      # development, with hot reload
npm run tauri build    # installers in host/target/release/bundle/
```

`npm run dev` alone opens the UI in a browser with synthetic data, handy for design work without the board.

The app talks to the bridge through `dualeye_core::Bridge`; its events (`waiting`, `connected`, `snapshot`, `board_log`, `disconnected`) are forwarded to the webview as `bridge`.

## Wire format

```json
{"v":1,"ts":1790358954,"cpu":{"temp_c":38.4,"load_pct":3.3,"clock_mhz":1187,"power_w":14.6},"gpu":{"temp_c":34.0,"load_pct":0.0,"clock_mhz":210,"power_w":21.2},"fans":[{"id":"cpu","rpm":3813},{"id":"gpu","rpm":0}]}
```

Parsed by `main/metrics_parser.c`; lines without any temperature are ignored, and the UI goes stale after 3 s without data.
