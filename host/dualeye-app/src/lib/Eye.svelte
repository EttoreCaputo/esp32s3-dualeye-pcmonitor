<!--
  One GC9A01 screen, laid out like create_temp_screen() in main/ui_watch.c:
  a 216 px usage ring on a 240 px panel and a centred column of labels.
  Everything is drawn in panel pixels and scaled as a whole.
-->
<script lang="ts">
  import { ARC_WIDTH, COLOR, DEVICES, FAN_PATH, LCD, USAGE_ARC_SIZE, WARN_PATH, type DeviceId, type Screen } from "./firmware";
  import type { BoardState } from "./monitor.svelte";

  let { id, screen, board, size, pixels = false }: {
    id: DeviceId;
    screen: Screen;
    board: BoardState;
    size: number;
    pixels?: boolean;
  } = $props();

  const dev = $derived(DEVICES[id]);
  const r = (USAGE_ARC_SIZE - ARC_WIDTH) / 2;
  const lit = $derived(board !== "off");
  const ui = $derived(board !== "off" && board !== "boot");
</script>

<div class="panel" class:lit style:--scale={size / LCD} style:width="{size}px" style:height="{size}px">
  <div class="fb" class:ui>
    <svg class="rings" viewBox="0 0 {LCD} {LCD}" aria-hidden="true">
      <circle cx={LCD / 2} cy={LCD / 2} {r} fill="none" stroke={dev.track} stroke-width={ARC_WIDTH} />
      {#if screen.usagePct > 0}
        <circle
          class="indicator"
          cx={LCD / 2}
          cy={LCD / 2}
          {r}
          fill="none"
          stroke={dev.accent}
          stroke-width={ARC_WIDTH}
          stroke-linecap="round"
          pathLength="100"
          stroke-dasharray="{screen.usagePct} 100"
          transform="rotate(-90 {LCD / 2} {LCD / 2})"
        />
      {/if}
    </svg>

    <div class="col">
      <div class="title-row" style:color={screen.labelColor}>
        {#if screen.warn}
          <svg class="warn" viewBox="0 0 512 512" aria-hidden="true"><path fill="currentColor" d={WARN_PATH} /></svg>
        {/if}
        <span class="title">{dev.title}</span>
      </div>
      <div class="value" style:color={screen.valueColor}>{screen.value}</div>
      <div class="row clock">
        <span>{screen.clock}</span>
        <span>{screen.watts}</span>
      </div>
      <div class="row load">
        <span style:color={screen.usageColor}>{screen.usage}</span>
        <svg class="fan" viewBox="0 0 512 512" aria-hidden="true"><path fill={COLOR.text} d={FAN_PATH} /></svg>
        <span class="rpm">{screen.rpm}</span>
      </div>
    </div>
  </div>
  {#if pixels}<div class="grid" aria-hidden="true"></div>{/if}
</div>

<style>
  .panel {
    position: relative;
    border-radius: 50%;
    overflow: hidden;
    /* An IPS panel with the backlight off is not black, it is a deep grey-green. */
    background: radial-gradient(circle at 50% 45%, #0d0f10 0%, #07090a 70%, #050606 100%);
    transition: background 600ms ease;
    isolation: isolate;
  }
  .panel.lit {
    background: #000;
  }

  .fb {
    position: absolute;
    left: 0;
    top: 0;
    width: 240px;
    height: 240px;
    transform: scale(var(--scale));
    transform-origin: 0 0;
    opacity: 0;
    transition: opacity 380ms ease;
  }
  .fb.ui {
    opacity: 1;
  }

  .rings {
    position: absolute;
    inset: 0;
    width: 240px;
    height: 240px;
    overflow: visible;
  }
  .indicator {
    transition: stroke-dasharray 320ms cubic-bezier(0.3, 0.7, 0.2, 1);
  }

  /* lv_obj_align(col, LV_ALIGN_CENTER, 0, 2) */
  .col {
    position: absolute;
    left: 50%;
    top: calc(50% + 2px);
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    white-space: nowrap;
    font-family: "Montserrat", sans-serif;
    color: #fff;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 10px;
    height: 15px;
  }
  .title {
    font: 700 12px/15px "Montserrat", sans-serif;
    letter-spacing: 1px;
  }
  .warn {
    width: 12px;
    height: 12px;
  }
  .value {
    font: 700 48px/49px "Montserrat", sans-serif;
    margin-bottom: 2px;
    /* LVGL has no tracking; Montserrat's default is already a touch wide at 48 px. */
    letter-spacing: -0.5px;
  }
  .row {
    display: flex;
    align-items: center;
    font: 500 14px/16px "Montserrat", sans-serif;
  }
  .clock {
    gap: 8px;
    margin-top: 4px;
    color: #9a9a9c;
  }
  .load {
    gap: 6px;
    margin-top: 2px;
    margin-bottom: 3px;
  }
  .fan {
    width: 16px;
    height: 16px;
  }

  /* 240 × 240 pixel lattice; only legible when drawn large (the loupe). */
  .grid {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background-image:
      linear-gradient(90deg, rgba(0, 0, 0, 0.7) 0 18%, transparent 18%),
      linear-gradient(0deg, rgba(0, 0, 0, 0.7) 0 18%, transparent 18%);
    background-size: calc(var(--scale) * 1px) calc(var(--scale) * 1px);
  }
</style>
