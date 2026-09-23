# ESP32-S3 DualEye PC Monitor

![Target](https://img.shields.io/badge/target-ESP32--S3-brightgreen)
![ESP-IDF](https://img.shields.io/badge/ESP--IDF-%3E%3D5.4-blue)
![LVGL](https://img.shields.io/badge/LVGL-9.3.0-orange)
![LCD](https://img.shields.io/badge/LCD-GC9A01%20×2-lightgrey)
![Host](https://img.shields.io/badge/host-CoolerControl-purple)

PC monitoring on **ESP32-S3 DualEye** (two 240×240 round displays): CPU/GPU temperatures and fans, Apple Watch–style UI.

A Python bridge on the host reads status from [CoolerControl](https://gitlab.com/coolercontrol/coolercontrol) and sends it to the board over USB Serial/JTAG as JSON.

| Piece | Role |
|-------|------|
| Firmware (`main/`) | LVGL UI + metrics ingest |
| Bridge (`host/`) | CoolerControl → UART/USB |

**Stack:** ESP-IDF ≥ 5.4 · [lvgl/lvgl](https://components.espressif.com/components/lvgl/lvgl) `9.3.0` · [espressif/esp_lcd_gc9a01](https://components.espressif.com/components/espressif/esp_lcd_gc9a01) `^2.0.4` · `pyserial`

CoolerControl token: copy `host/coolercontrol.env.example` → `host/coolercontrol.env` (do not commit).
