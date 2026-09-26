// Constants and formatting copied from main/ui_watch.c, so the mirror shows the
// same pixels the board does. Keep in sync when the firmware UI changes.

import type { Metrics } from "./monitor.svelte";

export const LCD = 240;
export const USAGE_ARC_SIZE = 216;
export const RING_GAP = 32;
export const ARC_WIDTH = 13;
export const GAUGE_ROTATION = 135;
export const GAUGE_SWEEP = 270;
export const TEMP_WARM_C = 80;
export const TEMP_HOT_C = 90;
export const MEM_HIGH_PCT = 90;
export const TEMP_MAX_C = 100;

export const COLOR = {
  text: "#FFFFFF",
  textDim: "#9A9A9C",
  cyan: "#3AE7ED",
  tempTrack: "#0B2C30",
  mem: "#5E8BFF",
  memTrack: "#141D3A",
  warm: "#F8A639",
  hot: "#F05354",
  stale: "#FFD60A",
} as const;

export const DEVICES = {
  cpu: { title: "CPU", memTitle: "RAM", accent: "#C4F06A", track: "#163012" },
  gpu: { title: "GPU", memTitle: "VRAM", accent: "#C86CF0", track: "#2A1238" },
} as const;
export type DeviceId = keyof typeof DEVICES;

/** `metrics_face_t`; the names are what goes on the wire. */
export type Face = "classic" | "rings" | "memory" | "gauge";
export type Faces = Record<DeviceId, Face>;
export const DEFAULT_FACES: Faces = { cpu: "classic", gpu: "classic" };
export const FACES: { id: Face; name: string; blurb: string }[] = [
  { id: "classic", name: "Classic", blurb: "Temperature, clock, power, fan" },
  { id: "rings", name: "Rings", blurb: "Load, temperature and memory rings" },
  { id: "memory", name: "Memory", blurb: "RAM or VRAM in use" },
  { id: "gauge", name: "Gauge", blurb: "Large temperature on a dial" },
];

/** The text and colours one round screen shows, as the `update_*` functions in ui_watch.c compute them. */
export type Screen = {
  face: Face;
  placeholder: boolean;
  title: string;
  value: string;
  clock: string;
  watts: string;
  usage: string;
  rpm: string;
  mem: string;
  memTotal: string;
  usagePct: number;
  tempPct: number;
  memPct: number;
  tempRing: string;
  labelColor: string;
  valueColor: string;
  usageColor: string;
  memColor: string;
  warn: boolean;
};

const cInt = (v: number) => Math.trunc(v + 0.5); // (int) (v + 0.5f)
/** `clamp_pct()` */
const pct = (v: number, max: number) => Math.min(100, Math.max(0, cInt((v / max) * 100)));
const pctText = (p: number | undefined) => (p === undefined ? "--%" : `${p}%`);

export function screenFor(
  id: DeviceId,
  face: Face,
  m: Metrics | undefined,
  stale: boolean,
  waiting: boolean,
  fan?: number,
): Screen {
  const dev = DEVICES[id];
  const accent = dev.accent;
  const mem = m?.mem && m.mem.total_mb > 0 ? m.mem : undefined;
  const memPct = mem ? pct(mem.used_mb, mem.total_mb) : undefined;
  const title = face === "memory" ? dev.memTitle : dev.title;
  const base = {
    face,
    title,
    clock: "-- GHz",
    watts: "-- W",
    usage: "--%",
    rpm: "--",
    mem: "--%",
    memTotal: "of -- GB",
    usagePct: 0,
    tempPct: 0,
    memPct: 0,
    tempRing: COLOR.cyan,
    memColor: COLOR.mem,
  };
  // The memory face only needs memory; the others need a temperature.
  if (waiting || (face === "memory" ? mem === undefined : m?.temp_c === undefined)) {
    return {
      ...base,
      placeholder: true,
      value: "—",
      labelColor: COLOR.textDim,
      valueColor: COLOR.textDim,
      usageColor: COLOR.textDim,
      memColor: COLOR.textDim,
      warn: false,
    };
  }

  if (face === "memory") {
    const p = memPct!;
    let labelColor: string = accent;
    let valueColor: string = COLOR.text;
    let warn = false;
    if (p >= MEM_HIGH_PCT) {
      labelColor = valueColor = COLOR.warm;
      warn = true;
    } else if (stale) {
      labelColor = COLOR.stale;
      valueColor = COLOR.textDim;
    }
    return {
      ...base,
      placeholder: false,
      value: (mem!.used_mb / 1024).toFixed(1),
      mem: pctText(p),
      memTotal: `of ${(mem!.total_mb / 1024).toFixed(0)} GB`,
      memPct: p,
      labelColor,
      valueColor,
      usageColor: accent,
      warn,
    };
  }

  // metrics_parser.c leaves absent fields at 0.
  const temp = m!.temp_c!;
  const usage = m!.load_pct ?? 0;
  let labelColor: string = accent;
  let valueColor: string = COLOR.text;
  let warn = false;
  if (temp >= TEMP_HOT_C) {
    labelColor = valueColor = COLOR.hot;
    warn = true;
  } else if (temp >= TEMP_WARM_C) {
    labelColor = valueColor = COLOR.warm;
    warn = true;
  } else if (stale) {
    labelColor = COLOR.stale;
    valueColor = COLOR.textDim;
  }
  const usagePct = pct(usage, 100);
  return {
    ...base,
    placeholder: false,
    value: `${cInt(temp)}°`,
    clock: `${((m!.clock_mhz ?? 0) / 1000).toFixed(1)} GHz`,
    watts: `${(m!.power_w ?? 0).toFixed(0)} W`,
    // Classic prints the load with "%.0f", the other faces the clamped ring value.
    usage: face === "classic" ? `${usage.toFixed(0)}%` : pctText(usagePct),
    rpm: fan === undefined ? "--" : String(fan),
    mem: pctText(memPct),
    usagePct,
    tempPct: pct(temp, TEMP_MAX_C),
    memPct: memPct ?? 0,
    tempRing: temp >= TEMP_HOT_C ? COLOR.hot : temp >= TEMP_WARM_C ? COLOR.warm : COLOR.cyan,
    labelColor,
    valueColor,
    usageColor: accent,
    memColor: memPct === undefined ? COLOR.textDim : COLOR.mem,
    warn,
  };
}

/** Colour a temperature the way the board would: accent, then warm, then hot. */
export function heatColor(id: DeviceId, temp: number | undefined): string {
  if (temp !== undefined && temp >= TEMP_HOT_C) return COLOR.hot;
  if (temp !== undefined && temp >= TEMP_WARM_C) return COLOR.warm;
  return DEVICES[id].accent;
}

// Font Awesome Free (CC BY 4.0): "fan" U+F863 and LV_SYMBOL_WARNING.
export const FAN_PATH =
  "M160 144c0-79.5 64.5-144 144-144 8.8 0 16 7.2 16 16l0 152.2c15-5.3 31.2-8.2 48-8.2 79.5 0 144 64.5 144 144 0 8.8-7.2 16-16 16l-152.2 0c5.3 15 8.2 31.2 8.2 48 0 79.5-64.5 144-144 144-8.8 0-16-7.2-16-16l0-152.2c-15 5.3-31.2 8.2-48 8.2-79.5 0-144-64.5-144-144 0-8.8 7.2-16 16-16l152.2 0c-5.3-15-8.2-31.2-8.2-48zm96 144a32 32 0 1 0 0-64 32 32 0 1 0 0 64z";
export const WARN_PATH =
  "M256 0c14.7 0 28.2 8.1 35.2 21l216 400c6.7 12.4 6.4 27.4-.8 39.5S486.1 480 472 480L40 480c-14.1 0-27.2-7.4-34.4-19.5s-7.5-27.1-.8-39.5l216-400c7-12.9 20.5-21 35.2-21zm0 352a32 32 0 1 0 0 64 32 32 0 1 0 0-64zm0-192c-18.2 0-32.7 15.5-31.4 33.7l7.4 104c.9 12.5 11.4 22.3 23.9 22.3 12.6 0 23-9.7 23.9-22.3l7.4-104c1.3-18.2-13.1-33.7-31.4-33.7z";
