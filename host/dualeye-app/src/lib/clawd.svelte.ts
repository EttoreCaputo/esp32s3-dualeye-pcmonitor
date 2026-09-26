// One clock for every Clawd on the page, ticking like `clawd_timer_cb()`.

import { CLAWD_TICK_MS } from "./firmware";

export const clawdClock = $state({ tick: 0 });

let started = false;

export function startClawdClock() {
  if (started) return;
  started = true;
  setInterval(() => clawdClock.tick++, CLAWD_TICK_MS);
}
