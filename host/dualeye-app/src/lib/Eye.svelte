<!--
  One GC9A01 screen, laid out like the create_*() face builders in main/ui_watch.c.
  Everything is drawn in panel pixels and scaled as a whole. Line heights are
  the LVGL fonts' own (bold 12: 9, montserrat 14: 16, bold 48: 35, bold 72: 52),
  which puts Montserrat's baseline where LVGL puts it.
-->
<script lang="ts">
  import {
    ARC_WIDTH,
    COLOR,
    DEVICES,
    FAN_PATH,
    GAUGE_ROTATION,
    GAUGE_SWEEP,
    LCD,
    RING_GAP,
    USAGE_ARC_SIZE,
    WARN_PATH,
    type DeviceId,
    type Screen,
  } from "./firmware";
  import type { BoardState } from "./monitor.svelte";

  let { id, screen, board, size, pixels = false }: {
    id: DeviceId;
    screen: Screen;
    board: BoardState;
    size: number;
    pixels?: boolean;
  } = $props();

  const dev = $derived(DEVICES[id]);
  const lit = $derived(board !== "off");
  const ui = $derived(board !== "off" && board !== "boot");
  const C = LCD / 2;
</script>

<!-- lv_arc: `value` 0–100 over `sweep` degrees, starting `rotation` degrees clockwise from 3 o'clock. -->
{#snippet arc(diameter: number, color: string, track: string, value: number, sweep = 360, rotation = 270)}
  {@const r = (diameter - ARC_WIDTH) / 2}
  {@const span = (sweep / 360) * 100}
  <g transform="rotate({rotation} {C} {C})">
    <circle
      cx={C}
      cy={C}
      {r}
      fill="none"
      stroke={track}
      stroke-width={ARC_WIDTH}
      stroke-linecap="round"
      pathLength="100"
      stroke-dasharray={sweep === 360 ? undefined : `${span} 100`}
    />
    {#if value > 0}
      <circle
        class="indicator"
        cx={C}
        cy={C}
        {r}
        fill="none"
        stroke={color}
        stroke-width={ARC_WIDTH}
        stroke-linecap="round"
        pathLength="100"
        stroke-dasharray="{(value / 100) * span} 100"
      />
    {/if}
  </g>
{/snippet}

{#snippet title(marginBottom: number)}
  <div class="title-row" style:color={screen.labelColor} style:margin-bottom="{marginBottom}px">
    {#if screen.warn}
      <svg class="warn" viewBox="0 0 512 512" aria-hidden="true"><path fill="currentColor" d={WARN_PATH} /></svg>
    {/if}
    <span class="title">{screen.title}</span>
  </div>
{/snippet}

<div class="panel" class:lit style:--scale={size / LCD} style:width="{size}px" style:height="{size}px">
  <div class="fb" class:ui>
    <svg class="rings" viewBox="0 0 {LCD} {LCD}" aria-hidden="true">
      {#if screen.face === "classic"}
        {@render arc(USAGE_ARC_SIZE, dev.accent, dev.track, screen.usagePct)}
      {:else if screen.face === "rings"}
        {@render arc(USAGE_ARC_SIZE, dev.accent, dev.track, screen.usagePct)}
        {@render arc(USAGE_ARC_SIZE - RING_GAP, screen.tempRing, COLOR.tempTrack, screen.tempPct)}
        {@render arc(USAGE_ARC_SIZE - 2 * RING_GAP, COLOR.mem, COLOR.memTrack, screen.memPct)}
      {:else if screen.face === "memory"}
        {@render arc(USAGE_ARC_SIZE, COLOR.mem, COLOR.memTrack, screen.memPct)}
      {:else}
        {@render arc(USAGE_ARC_SIZE, screen.tempRing, COLOR.tempTrack, screen.tempPct, GAUGE_SWEEP, GAUGE_ROTATION)}
      {/if}
    </svg>

    {#if screen.face === "classic"}
      <div class="col" style:--y="2px">
        {@render title(10)}
        <div class="value" style:color={screen.valueColor} style:margin-bottom="2px">{screen.value}</div>
        <div class="row dim" style:gap="8px" style:margin-top="4px">
          <span>{screen.clock}</span>
          <span>{screen.watts}</span>
        </div>
        <div class="row" style:gap="6px" style:margin="2px 0 3px">
          <span style:color={screen.usageColor}>{screen.usage}</span>
          <svg class="fan" viewBox="0 0 512 512" aria-hidden="true"><path fill={COLOR.text} d={FAN_PATH} /></svg>
          <span>{screen.rpm}</span>
        </div>
      </div>
    {:else if screen.face === "rings"}
      <div class="col" style:--y="2px">
        {@render title(6)}
        <div class="value" style:color={screen.valueColor}>{screen.value}</div>
        <div class="row" style:gap="8px" style:margin-top="6px">
          <span style:color={screen.usageColor}>{screen.usage}</span>
          <span style:color={screen.memColor}>{screen.mem}</span>
        </div>
      </div>
    {:else if screen.face === "memory"}
      <div class="col" style:--y="2px">
        {@render title(10)}
        <div class="value" style:color={screen.valueColor}>{screen.value}</div>
        <div class="row dim" style:margin-top="6px">{screen.memTotal}</div>
        <div class="row" style:color={screen.memColor} style:margin-top="2px">{screen.mem}</div>
      </div>
    {:else}
      <div class="col" style:--y="-4px">
        {@render title(8)}
        <div class="value big" style:color={screen.valueColor}>{screen.value}</div>
      </div>
      <div class="col" style:--y="84px">
        <div class="row" style:color={screen.usageColor}>{screen.usage}</div>
      </div>
    {/if}
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
    transition:
      stroke-dasharray 320ms cubic-bezier(0.3, 0.7, 0.2, 1),
      stroke 320ms ease;
  }

  /* lv_obj_align(col, LV_ALIGN_CENTER, 0, y) */
  .col {
    position: absolute;
    left: 50%;
    top: calc(50% + var(--y));
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    white-space: nowrap;
    font-family: "Montserrat", sans-serif;
    color: #fff;
  }
  /* The hidden warning icon takes no room in LVGL's flex layout. */
  .title-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .title {
    font: 700 12px/9px "Montserrat", sans-serif;
    letter-spacing: 1px;
  }
  .warn {
    width: 12px;
    height: 12px;
    margin: 1.5px 0;
  }
  .value {
    font: 700 48px/35px "Montserrat", sans-serif;
    /* LVGL has no tracking; Montserrat's default is already a touch wide at 48 px. */
    letter-spacing: -0.5px;
  }
  .value.big {
    font-size: 72px;
    line-height: 52px;
    letter-spacing: -0.75px;
  }
  .row {
    display: flex;
    align-items: center;
    font: 500 14px/16px "Montserrat", sans-serif;
  }
  .dim {
    color: #9a9a9c;
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
