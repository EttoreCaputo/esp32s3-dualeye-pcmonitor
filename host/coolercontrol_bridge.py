#!/usr/bin/env python3
"""Forward CoolerControl SSE status to the DualEye board as one JSON line.

coolercontrold listens on localhost only. The ESP32 is attached over USB and
has no route to that address, so this process (on the same PC) keeps the SSE
connection open and writes a compact snapshot to the board's serial port.

Close idf.py monitor before starting: it owns the same port.

  python3 host/coolercontrol_bridge.py --port /dev/ttyACM0

Put a read-only access token in host/coolercontrol.env (see the example file).
That file is local and is not committed. CLI flags and environment variables
override it.
"""

from __future__ import annotations

import argparse
import base64
import glob
import json
import os
import sys
import threading
import time
import urllib.error
import urllib.request
from typing import Any

DEFAULT_BASE = "http://127.0.0.1:11987"
CONFIG_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "coolercontrol.env")


def load_local_config() -> None:
    """Load host/coolercontrol.env into the environment without overriding it."""
    if not os.path.isfile(CONFIG_PATH):
        return
    with open(CONFIG_PATH, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            key = key.strip()
            value = value.strip().strip("\"'")
            if key and value and key not in os.environ:
                os.environ[key] = value


def latest_status(device: dict[str, Any]) -> dict[str, Any]:
    history = device.get("status_history") or []
    if not history or not isinstance(history[-1], dict):
        return {}
    return history[-1]


def find_device(devices: list[dict[str, Any]], kind: str) -> dict[str, Any] | None:
    for device in devices:
        if device.get("type") == kind:
            return device
    return None


def named_temp(status: dict[str, Any], name: str) -> float | None:
    for entry in status.get("temps") or []:
        if entry.get("name") == name and isinstance(entry.get("temp"), (int, float)):
            return float(entry["temp"])
    return None


def max_temp(status: dict[str, Any], skip_hotspot: bool = False) -> float | None:
    values = []
    for entry in status.get("temps") or []:
        name = str(entry.get("name") or "")
        if skip_hotspot and "hotspot" in name.lower():
            continue
        if isinstance(entry.get("temp"), (int, float)):
            values.append(float(entry["temp"]))
    return max(values) if values else None


def find_channel(status: dict[str, Any], name: str) -> dict[str, Any] | None:
    for channel in status.get("channels") or []:
        if channel.get("name") == name:
            return channel
    return None


def channel_duty(status: dict[str, Any], name: str) -> float | None:
    channel = find_channel(status, name)
    if channel is None or not isinstance(channel.get("duty"), (int, float)):
        return None
    return float(channel["duty"])


def channel_freq(status: dict[str, Any], name: str) -> float | None:
    channel = find_channel(status, name)
    if channel is None or not isinstance(channel.get("freq"), (int, float)):
        return None
    return float(channel["freq"])


def channel_watts(status: dict[str, Any]) -> float | None:
    channels = [c for c in (status.get("channels") or []) if isinstance(c.get("watts"), (int, float))]
    preferred = [c for c in channels if "power" in str(c.get("name") or "").lower()]
    pool = preferred or channels
    if not pool:
        return None
    return float(pool[0]["watts"])


def fan_rpms(status: dict[str, Any]) -> list[int]:
    rpms = []
    for channel in status.get("channels") or []:
        if isinstance(channel.get("rpm"), (int, float)):
            rpms.append(int(round(float(channel["rpm"]))))
    return rpms


def _round1(value: float) -> float:
    return round(float(value), 1)


def _device_payload(temp: float | None, load: float | None, clock_mhz: float | None, power_w: float | None) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    if temp is not None:
        payload["temp_c"] = _round1(temp)
    if load is not None:
        payload["load_pct"] = _round1(load)
    if clock_mhz is not None:
        payload["clock_mhz"] = int(round(clock_mhz))
    if power_w is not None:
        payload["power_w"] = _round1(power_w)
    return payload


def extract_snapshot(status_payload: dict[str, Any]) -> dict[str, Any] | None:
    """Reduce a CoolerControl `status` event to the board's snapshot line."""
    devices = status_payload.get("devices")
    if not isinstance(devices, list):
        return None

    cpu = find_device(devices, "CPU")
    gpu = find_device(devices, "GPU")
    cpu_status = latest_status(cpu) if cpu else {}
    gpu_status = latest_status(gpu) if gpu else {}

    cpu_obj = _device_payload(
        max_temp(cpu_status),
        channel_duty(cpu_status, "CPU Load"),
        channel_freq(cpu_status, "CPU Freq Avg"),
        channel_watts(cpu_status),
    )
    gpu_temp = named_temp(gpu_status, "GPU Temp")
    if gpu_temp is None:
        gpu_temp = max_temp(gpu_status, skip_hotspot=True)
    gpu_obj = _device_payload(
        gpu_temp,
        channel_duty(gpu_status, "GPU Load"),
        channel_freq(gpu_status, "freq_graphics"),
        channel_watts(gpu_status),
    )

    case_rpms: list[int] = []
    for device in devices:
        if device.get("type") in ("CPU", "GPU"):
            continue
        case_rpms.extend(fan_rpms(latest_status(device)))
    gpu_fans = fan_rpms(gpu_status)

    fans = []
    if case_rpms:
        fans.append({"id": "cpu", "rpm": max(case_rpms)})
    if gpu_fans:
        fans.append({"id": "gpu", "rpm": max(gpu_fans)})

    if "temp_c" not in cpu_obj and "temp_c" not in gpu_obj:
        return None

    snapshot: dict[str, Any] = {"v": 1, "ts": int(time.time())}
    if cpu_obj:
        snapshot["cpu"] = cpu_obj
    if gpu_obj:
        snapshot["gpu"] = gpu_obj
    if fans:
        snapshot["fans"] = fans
    return snapshot


def normalize_cookie(raw: str) -> str:
    cookie = raw.strip()
    if cookie.lower().startswith("cookie:"):
        cookie = cookie.split(":", 1)[1].strip()
    if "=" not in cookie:
        cookie = "cc=" + cookie
    return cookie


def detect_port() -> str | None:
    found: list[str] = []
    for pattern in ("/dev/ttyACM*", "/dev/cu.usbmodem*", "/dev/ttyUSB*"):
        found.extend(glob.glob(pattern))
    found = sorted(set(found))
    if len(found) == 1:
        return found[0]
    return None


def build_opener(base: str, token: str | None, cookie: str | None, user: str, password: str | None):
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor())
    if password and not cookie and not token:
        basic = base64.b64encode(f"{user}:{password}".encode()).decode("ascii")
        login = urllib.request.Request(
            base.rstrip("/") + "/login",
            headers={"Authorization": f"Basic {basic}", "User-Agent": "dualeye-bridge/1"},
        )
        with opener.open(login, timeout=10) as response:
            response.read()
    return opener


def request_headers(token: str | None, cookie: str | None) -> dict[str, str]:
    headers = {
        "Accept": "text/event-stream",
        "Cache-Control": "no-cache",
        "User-Agent": "dualeye-bridge/1",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if cookie:
        headers["Cookie"] = normalize_cookie(cookie)
    return headers


def iter_sse(response):
    event = "message"
    data: list[str] = []
    while True:
        raw = response.readline()
        if not raw:
            return
        line = raw.decode("utf-8", "replace").rstrip("\r\n")
        if line == "":
            if data:
                yield event, "\n".join(data)
            event = "message"
            data = []
            continue
        if line.startswith(":"):
            continue
        if line.startswith("event:"):
            event = line[6:].strip()
        elif line.startswith("data:"):
            data.append(line[5:].lstrip())


def port_permission_denied(exc: BaseException) -> bool:
    if isinstance(exc, PermissionError):
        return True
    return "permission denied" in str(exc).lower()


def open_serial(port: str):
    import serial

    ser = serial.Serial()
    ser.port = port
    ser.baudrate = 115200
    ser.timeout = 0.2
    ser.dtr = False
    ser.rts = False
    ser.open()
    ser.dtr = False
    ser.rts = False
    return ser


def drain_serial(ser, stop: threading.Event) -> None:
    while not stop.is_set():
        try:
            data = ser.read(256)
        except Exception:
            return
        if data:
            sys.stderr.write(data.decode("utf-8", "replace"))
            sys.stderr.flush()


def forward(args, ser) -> None:
    opener = build_opener(args.base, args.token, args.cookie, args.user, args.password)
    url = args.base.rstrip("/") + "/sse"
    req = urllib.request.Request(url, headers=request_headers(args.token, args.cookie))
    print(f"connecting {url}", flush=True)
    with opener.open(req, timeout=60) as response:
        for event, data in iter_sse(response):
            if event != "status":
                continue
            try:
                payload = json.loads(data)
            except json.JSONDecodeError as exc:
                print(f"bad status json: {exc}", file=sys.stderr)
                continue
            snapshot = extract_snapshot(payload)
            if snapshot is None:
                continue
            line = json.dumps(snapshot, separators=(",", ":")) + "\n"
            ser.write(line.encode("utf-8"))
            ser.flush()
            cpu = snapshot.get("cpu", {})
            gpu = snapshot.get("gpu", {})
            print(
                "cpu {temp}C {load}% {mhz}MHz {watts}W | gpu {gtemp}C {gload}% {gmhz}MHz {gwatts}W".format(
                    temp=cpu.get("temp_c", "—"),
                    load=cpu.get("load_pct", "—"),
                    mhz=cpu.get("clock_mhz", "—"),
                    watts=cpu.get("power_w", "—"),
                    gtemp=gpu.get("temp_c", "—"),
                    gload=gpu.get("load_pct", "—"),
                    gmhz=gpu.get("clock_mhz", "—"),
                    gwatts=gpu.get("power_w", "—"),
                ),
                flush=True,
            )


def run_dry(args) -> None:
    opener = build_opener(args.base, args.token, args.cookie, args.user, args.password)
    url = args.base.rstrip("/") + "/sse"
    req = urllib.request.Request(url, headers=request_headers(args.token, args.cookie))
    print(f"connecting {url}", flush=True)
    with opener.open(req, timeout=60) as response:
        for event, data in iter_sse(response):
            if event != "status":
                continue
            snapshot = extract_snapshot(json.loads(data))
            if snapshot is None:
                continue
            print(json.dumps(snapshot, separators=(",", ":")))
            return


def run(args) -> None:
    print(f"opening {args.port}", flush=True)
    ser = open_serial(args.port)
    stop = threading.Event()
    thread = threading.Thread(target=drain_serial, args=(ser, stop), daemon=True)
    thread.start()
    delay = 1.0
    try:
        if args.boot_wait > 0:
            time.sleep(args.boot_wait)
        while True:
            try:
                forward(args, ser)
                delay = 1.0
                print("sse closed, reconnecting", file=sys.stderr)
            except urllib.error.HTTPError:
                raise
            except Exception as exc:
                import serial

                if isinstance(exc, serial.SerialException):
                    raise
                print(f"{exc}, retry in {delay:.0f}s", file=sys.stderr)
            time.sleep(delay)
            delay = min(delay * 2, 15.0)
    finally:
        stop.set()
        ser.close()


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Forward CoolerControl SSE status to the DualEye board")
    parser.add_argument("--base", default=os.environ.get("COOLERCONTROL_URL", DEFAULT_BASE))
    parser.add_argument("--port", default=os.environ.get("DUALEYE_PORT") or detect_port())
    parser.add_argument("--token", default=os.environ.get("COOLERCONTROL_TOKEN"))
    parser.add_argument("--cookie", default=os.environ.get("COOLERCONTROL_COOKIE"))
    parser.add_argument("--user", default=os.environ.get("COOLERCONTROL_USER", "CCAdmin"))
    parser.add_argument("--password", default=os.environ.get("COOLERCONTROL_PASSWORD"))
    parser.add_argument("--boot-wait", type=float, default=2.0, help="seconds to wait after opening USB")
    parser.add_argument("--dry-run", action="store_true", help="print one snapshot and exit, no serial")
    parser.add_argument("--self-test", action="store_true", help="check the status mapping and exit")
    args = parser.parse_args(argv)
    if not args.self_test and not args.dry_run and not args.port:
        parser.error("serial port not found; pass --port /dev/ttyACM0")
    return args


def self_test() -> None:
    payload = {
        "devices": [
            {
                "type": "CPU",
                "status_history": [
                    {
                        "temps": [{"name": "temp1", "temp": 37.0}, {"name": "temp6", "temp": 38.0}, {"name": "temp38", "temp": 34.0}],
                        "channels": [
                            {"name": "CPU Load", "duty": 1.8113555908203125},
                            {"name": "power0", "watts": 14.596215000001393},
                            {"name": "CPU Freq Avg", "freq": 1572},
                            {"name": "CPU Freq Max", "freq": 4601},
                        ],
                    }
                ],
            },
            {
                "type": "GPU",
                "status_history": [
                    {
                        "temps": [{"name": "GPU Temp", "temp": 31.0}, {"name": "GPU Temp Hotspot", "temp": 41.0}],
                        "channels": [
                            {"name": "fan1", "rpm": 0, "duty": 0.0},
                            {"name": "fan2", "rpm": 0, "duty": 0.0},
                            {"name": "GPU Load", "duty": 0.0},
                            {"name": "GPU Power", "watts": 20.0},
                            {"name": "freq_graphics", "freq": 210},
                        ],
                    }
                ],
            },
            {
                "type": "Hwmon",
                "status_history": [
                    {
                        "channels": [
                            {"name": "fan1", "rpm": 791, "duty": 40.0},
                            {"name": "fan2", "rpm": 508, "duty": 25.0},
                            {"name": "fan7", "rpm": 3770, "duty": 39.0},
                        ]
                    }
                ],
            },
            {"type": "Liquidctl", "status_history": [{}]},
        ]
    }
    snapshot = extract_snapshot(payload)
    assert snapshot is not None
    assert snapshot["cpu"] == {"temp_c": 38.0, "load_pct": 1.8, "clock_mhz": 1572, "power_w": 14.6}
    assert snapshot["gpu"] == {"temp_c": 31.0, "load_pct": 0.0, "clock_mhz": 210, "power_w": 20.0}
    assert snapshot["fans"] == [{"id": "cpu", "rpm": 3770}, {"id": "gpu", "rpm": 0}]
    assert normalize_cookie("abc") == "cc=abc"
    assert normalize_cookie("Cookie: cc=abc") == "cc=abc"
    print("self-test ok")


def main(argv: list[str]) -> int:
    load_local_config()
    args = parse_args(argv)
    if args.self_test:
        self_test()
        return 0
    if args.dry_run:
        try:
            run_dry(args)
        except urllib.error.HTTPError as exc:
            if exc.code in (401, 403):
                print(
                    "CoolerControl refused the request. Set COOLERCONTROL_TOKEN in host/coolercontrol.env.",
                    file=sys.stderr,
                )
            else:
                print(f"http {exc.code}: {exc.reason}", file=sys.stderr)
            return 1
        return 0

    delay = 1.0
    while True:
        try:
            run(args)
            return 0
        except KeyboardInterrupt:
            print("\nstopped")
            return 0
        except ImportError:
            print("pyserial is missing: python3 -m pip install -r host/requirements.txt", file=sys.stderr)
            return 1
        except urllib.error.HTTPError as exc:
            if exc.code in (401, 403):
                print(
                    "CoolerControl refused the request. Set COOLERCONTROL_TOKEN in host/coolercontrol.env.",
                    file=sys.stderr,
                )
                return 1
            print(f"http {exc.code}, retry in {delay:.0f}s", file=sys.stderr)
        except Exception as exc:
            if port_permission_denied(exc):
                print(exc, file=sys.stderr)
                print(
                    "The serial port is not writable. On Ubuntu add this user to dialout, then log out and back in:\n"
                    "  sudo usermod -aG dialout \"$USER\"\n"
                    "Until then: sudo chmod a+rw /dev/ttyACM0",
                    file=sys.stderr,
                )
                return 1
            print(f"{exc}, retry in {delay:.0f}s", file=sys.stderr)
        time.sleep(delay)
        delay = min(delay * 2, 15.0)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
