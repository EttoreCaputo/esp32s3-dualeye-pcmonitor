<script lang="ts">
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import Chart from "./Chart.svelte";
  import { DEVICES, heatColor, type DeviceId } from "./firmware";
  import type { Metrics, Sample } from "./monitor.svelte";

  let { id, metrics, fan, samples, lcd }: {
    id: DeviceId;
    metrics: Metrics | undefined;
    fan: number | undefined;
    samples: Sample[];
    lcd: number;
  } = $props();

  const dev = $derived(DEVICES[id]);
  const opts = { duration: 700, easing: cubicOut };
  const temp = new Tween(0, opts);
  const load = new Tween(0, opts);
  const clock = new Tween(0, opts);
  const power = new Tween(0, opts);
  const rpm = new Tween(0, opts);

  $effect(() => {
    if (metrics?.temp_c !== undefined) temp.target = metrics.temp_c;
    if (metrics?.load_pct !== undefined) load.target = metrics.load_pct;
    if (metrics?.clock_mhz !== undefined) clock.target = metrics.clock_mhz;
    if (metrics?.power_w !== undefined) power.target = metrics.power_w;
    if (fan !== undefined) rpm.target = fan;
  });

  const heat = $derived(heatColor(id, metrics?.temp_c));
  const hasTemp = $derived(metrics?.temp_c !== undefined);
  const tempOf = (s: Sample) => (id === "cpu" ? s.cpuT : s.gpuT);
  const loadOf = (s: Sample) => (id === "cpu" ? s.cpuL : s.gpuL);

  const range = $derived.by(() => {
    const v = samples.map(tempOf).filter((t): t is number => t !== undefined);
    return v.length ? { min: Math.min(...v), max: Math.max(...v) } : null;
  });
</script>

<article class="card" style:--accent={dev.accent} style:--heat={heat}>
  <header>
    <span class="dot"></span>
    <h2>{dev.title}</h2>
    <span class="lcd">LCD {lcd}</span>
    <div class="legend">
      <span><i class="l-temp"></i>Temp</span>
      <span><i class="l-load"></i>Load</span>
    </div>
    {#if range}
      <span class="range">
        <span>min <b>{range.min.toFixed(0)}°</b></span>
        <span>max <b>{range.max.toFixed(0)}°</b></span>
      </span>
    {/if}
  </header>

  <div class="body">
    <div class="stats">
      <div class="temp" class:empty={!hasTemp}>
        {#if hasTemp}
          <span class="big">{temp.current.toFixed(1)}</span><span class="unit">°C</span>
        {:else}
          <span class="big">—</span>
        {/if}
      </div>
      <dl>
        <div>
          <dt>Load</dt>
          <dd class="accent">{metrics?.load_pct === undefined ? "—" : `${load.current.toFixed(0)}`}<small>%</small></dd>
        </div>
        <div>
          <dt>Clock</dt>
          <dd>{metrics?.clock_mhz === undefined ? "—" : (clock.current / 1000).toFixed(2)}<small>GHz</small></dd>
        </div>
        <div>
          <dt>Power</dt>
          <dd>{metrics?.power_w === undefined ? "—" : power.current.toFixed(1)}<small>W</small></dd>
        </div>
        <div>
          <dt>Fan</dt>
          <dd>{fan === undefined ? "—" : Math.round(rpm.current).toLocaleString("en-US")}<small>rpm</small></dd>
        </div>
      </dl>
    </div>
    <div class="plot">
      <Chart {samples} temp={tempOf} load={loadOf} color={dev.accent} />
    </div>
  </div>
</article>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px 14px;
    border-radius: 22px;
    background:
      radial-gradient(120% 140% at 0% 0%, color-mix(in srgb, var(--accent) 7%, transparent), transparent 55%),
      linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.015));
    border: 1px solid var(--line);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.05),
      0 20px 50px -30px rgba(0, 0, 0, 0.9);
    min-width: 0;
    overflow: hidden;
  }
  .card::before {
    content: "";
    position: absolute;
    inset: 0 0 auto;
    height: 1px;
    background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent) 45%, transparent), transparent);
    opacity: 0.6;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--heat);
    box-shadow: 0 0 12px var(--heat);
    transition: background 600ms, box-shadow 600ms;
  }
  h2 {
    margin: 0;
    font: 650 13px/1 var(--sans);
    letter-spacing: 0.14em;
  }
  .lcd {
    font: 500 10px/1 var(--mono);
    letter-spacing: 0.08em;
    color: var(--faint);
    padding: 4px 7px;
    border: 1px solid var(--line);
    border-radius: 999px;
  }
  .range {
    display: flex;
    gap: 14px;
    padding-left: 14px;
    border-left: 1px solid var(--line);
    font: 500 10.5px/1 var(--mono);
    color: var(--faint);
    letter-spacing: 0.04em;
  }
  .range b {
    color: var(--dim);
    font-weight: 600;
  }

  .body {
    display: grid;
    grid-template-columns: minmax(190px, 230px) 1fr;
    gap: 22px;
    flex: 1;
    min-height: 0;
  }

  .temp {
    display: flex;
    align-items: flex-start;
    gap: 4px;
    color: var(--text);
    margin: -4px 0 14px;
  }
  .big {
    font: 300 58px/1 var(--sans);
    letter-spacing: -0.045em;
    font-variant-numeric: tabular-nums;
    color: color-mix(in srgb, var(--heat) 18%, var(--text));
    transition: color 600ms;
  }
  .temp.empty .big {
    color: var(--faint);
  }
  .unit {
    font: 500 15px/1 var(--sans);
    color: var(--dim);
    margin-top: 8px;
  }

  dl {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 16px;
    margin: 0;
  }
  dt {
    font: 600 9.5px/1 var(--mono);
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
    margin-bottom: 6px;
  }
  dd {
    margin: 0;
    font: 550 16px/1 var(--sans);
    font-variant-numeric: tabular-nums;
    color: var(--text);
    letter-spacing: -0.01em;
  }
  dd.accent {
    color: var(--accent);
  }
  dd small {
    font-size: 10.5px;
    font-weight: 500;
    color: var(--faint);
    margin-left: 3px;
    letter-spacing: 0.02em;
  }

  .plot {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .legend {
    margin-left: auto;
    display: flex;
    gap: 14px;
    font: 500 10.5px/1 var(--sans);
    color: var(--faint);
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .legend i {
    display: inline-block;
    width: 12px;
    height: 2px;
    border-radius: 2px;
  }
  .l-temp {
    background: #f4f5f7;
  }
  .l-load {
    height: 8px !important;
    background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 60%, transparent), transparent);
    border-top: 1.5px solid var(--accent);
    border-radius: 1px !important;
  }
</style>
