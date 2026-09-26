<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { monitor, type PortInfo, type Reading } from "./monitor.svelte";

  let { open = $bindable(false) }: { open: boolean } = $props();

  type Tab = "connection" | "sensors" | "console";
  let tab = $state<Tab>("connection");
  let ports = $state<PortInfo[]>([]);
  let readings = $state<Reading[]>([]);
  let consoleEl = $state<HTMLElement>();
  let follow = $state(true);

  $effect(() => {
    if (!open || tab !== "connection") return;
    const load = () => monitor.listPorts().then((p) => (ports = p));
    load();
    const id = setInterval(load, 2000);
    return () => clearInterval(id);
  });

  $effect(() => {
    if (!open || tab !== "sensors") return;
    let alive = true;
    const load = async () => {
      const r = await monitor.readings();
      if (alive) readings = r;
    };
    load();
    const id = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  });

  $effect(() => {
    void monitor.logs.length;
    if (follow && consoleEl) queueMicrotask(() => consoleEl && (consoleEl.scrollTop = consoleEl.scrollHeight));
  });

  const groups = $derived.by(() => {
    const m = new Map<string, Reading[]>();
    for (const r of readings) m.set(r.source, [...(m.get(r.source) ?? []), r]);
    return [...m.entries()];
  });

  function onKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") open = false;
  }

  function scrolled() {
    if (!consoleEl) return;
    follow = consoleEl.scrollHeight - consoleEl.scrollTop - consoleEl.clientHeight < 24;
  }

  const hex = (n: number) => n.toString(16).padStart(4, "0");
  const fmt = (r: Reading) => (r.unit === "RPM" || r.unit === "%" ? r.value.toFixed(0) : r.value.toFixed(1));
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <button class="scrim" transition:fade={{ duration: 200 }} onclick={() => (open = false)} aria-label="Close settings"></button>
  <aside class="drawer" transition:fly={{ x: 40, duration: 320, easing: cubicOut, opacity: 0 }} aria-label="Settings">
    <nav>
      {#each [["connection", "Connection"], ["sensors", "Sensors"], ["console", "Console"]] as [key, label] (key)}
        <button class:active={tab === key} onclick={() => (tab = key as Tab)}>{label}</button>
      {/each}
      <span class="indicator" style:--i={["connection", "sensors", "console"].indexOf(tab)}></span>
    </nav>

    <div class="content">
      {#if tab === "connection"}
        <section>
          <h3>Serial port</h3>
          <p class="hint">The board is found automatically by its Espressif USB ID. Pin a port only if you have more than one.</p>
          <div class="ports">
            <label class="port" class:checked={monitor.portSetting === null}>
              <input type="radio" name="port" checked={monitor.portSetting === null} onchange={() => monitor.setPort(null)} />
              <span class="radio"></span>
              <span class="pname">Automatic</span>
              <span class="pmeta">{monitor.portSetting === null && monitor.port ? monitor.port : "303a:*"}</span>
            </label>
            {#each ports as p (p.name)}
              <label class="port" class:checked={monitor.portSetting === p.name}>
                <input type="radio" name="port" checked={monitor.portSetting === p.name} onchange={() => monitor.setPort(p.name)} />
                <span class="radio"></span>
                <span class="pname">{p.name}</span>
                <span class="pmeta">{hex(p.vid)}:{hex(p.pid)}{p.is_board ? " · DualEye" : ""}</span>
              </label>
            {:else}
              <p class="empty">No USB serial ports found.</p>
            {/each}
          </div>
        </section>

        {#if monitor.message && monitor.link !== "connected"}
          <section class="alert" class:error={monitor.link === "offline"}>
            <h3>{monitor.link === "offline" ? "Connection lost" : "Waiting"}</h3>
            <p class="mono">{monitor.message}</p>
            {#if monitor.permissionDenied}
              <p class="hint">Linux: add yourself to the <code>dialout</code> group, then log out and back in.</p>
              <pre>sudo usermod -aG dialout "$USER"</pre>
            {/if}
          </section>
        {/if}

        <section>
          <h3>Stream</h3>
          <dl class="facts">
            <div><dt>Rate</dt><dd>1 Hz</dd></div>
            <div><dt>Baud</dt><dd>115200</dd></div>
            <div><dt>Format</dt><dd>JSON line · v1</dd></div>
            <div><dt>Stale after</dt><dd>3 s</dd></div>
          </dl>
        </section>
      {:else if tab === "sensors"}
        <p class="hint">Every raw value the host can read. The board gets the CPU average, the first GPU and the fastest fan of each kind.</p>
        {#each groups as [source, list] (source)}
          <section class="group">
            <h3>{source}</h3>
            <ul>
              {#each list as r (r.label)}
                <li><span>{r.label}</span><b>{fmt(r)}<small>{r.unit}</small></b></li>
              {/each}
            </ul>
          </section>
        {:else}
          <p class="empty">Reading sensors…</p>
        {/each}
      {:else}
        <div class="console" bind:this={consoleEl} onscroll={scrolled}>
          {#each monitor.logs as line, i (i)}
            <div class="log" class:warn={line.startsWith("W ")} class:err={line.startsWith("E ")}>{line}</div>
          {:else}
            <p class="empty">Nothing logged by the board yet.</p>
          {/each}
        </div>
      {/if}
    </div>
  </aside>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    border: 0;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(3px);
    cursor: default;
  }
  .drawer {
    position: fixed;
    z-index: 50;
    top: 10px;
    right: 10px;
    bottom: 10px;
    width: min(400px, calc(100vw - 20px));
    display: flex;
    flex-direction: column;
    border-radius: 20px;
    background: rgba(17, 18, 21, 0.86);
    backdrop-filter: blur(28px) saturate(1.4);
    border: 1px solid rgba(255, 255, 255, 0.09);
    box-shadow:
      0 40px 80px -20px rgba(0, 0, 0, 0.8),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
    overflow: hidden;
  }

  nav {
    position: relative;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    margin: 14px;
    padding: 3px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--line);
  }
  nav button {
    position: relative;
    z-index: 1;
    height: 30px;
    border: 0;
    background: none;
    color: var(--dim);
    font: 550 12px/1 var(--sans);
    cursor: pointer;
    transition: color 200ms;
  }
  nav button.active {
    color: var(--text);
  }
  .indicator {
    position: absolute;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc((100% - 6px) / 3);
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.08);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
    transform: translateX(calc(var(--i) * 100%));
    transition: transform 320ms cubic-bezier(0.3, 0.8, 0.2, 1);
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 4px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 22px;
    min-height: 0;
  }
  h3 {
    margin: 0 0 8px;
    font: 600 10px/1 var(--mono);
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .hint {
    margin: 0 0 12px;
    font: 400 12.5px/1.5 var(--sans);
    color: var(--dim);
  }
  .empty {
    color: var(--faint);
    font: 400 12.5px/1.5 var(--sans);
    margin: 0;
  }
  .mono,
  pre,
  code {
    font-family: var(--mono);
    font-size: 11.5px;
  }
  pre {
    margin: 0;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid var(--line);
    color: var(--text);
    user-select: text;
  }

  .ports {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .port {
    display: grid;
    grid-template-columns: 18px 1fr auto;
    align-items: center;
    gap: 10px;
    padding: 11px 12px;
    border-radius: 12px;
    border: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.02);
    cursor: pointer;
    transition:
      border-color 200ms,
      background 200ms;
  }
  .port:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .port.checked {
    border-color: color-mix(in srgb, var(--cpu) 45%, transparent);
    background: color-mix(in srgb, var(--cpu) 6%, transparent);
  }
  .port input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .port:has(input:focus-visible) {
    outline: 1.5px solid var(--cpu);
  }
  .radio {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid var(--faint);
    transition: all 200ms;
  }
  .port.checked .radio {
    border: 4px solid var(--cpu);
  }
  .pname {
    font: 550 13px/1.2 var(--sans);
  }
  .pmeta {
    font: 500 10.5px/1 var(--mono);
    color: var(--faint);
  }

  .alert {
    padding: 14px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--stale) 7%, transparent);
    border: 1px solid color-mix(in srgb, var(--stale) 22%, transparent);
  }
  .alert.error {
    background: color-mix(in srgb, var(--hot) 8%, transparent);
    border-color: color-mix(in srgb, var(--hot) 28%, transparent);
  }
  .alert p {
    margin: 0 0 10px;
    color: var(--text);
  }

  .facts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    margin: 0;
    border-radius: 12px;
    overflow: hidden;
    border: 1px solid var(--line);
    background: var(--line);
  }
  .facts div {
    padding: 11px 12px;
    background: #121316;
  }
  .facts dt {
    font: 500 10.5px/1 var(--sans);
    color: var(--faint);
    margin-bottom: 6px;
  }
  .facts dd {
    margin: 0;
    font: 550 12.5px/1 var(--mono);
  }

  .group ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border-radius: 12px;
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .group li {
    display: flex;
    justify-content: space-between;
    padding: 8px 12px;
    font: 450 12.5px/1.3 var(--sans);
    color: var(--dim);
  }
  .group li:nth-child(odd) {
    background: rgba(255, 255, 255, 0.02);
  }
  .group b {
    font: 550 12px/1.3 var(--mono);
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .group small {
    color: var(--faint);
    margin-left: 4px;
    font-weight: 500;
  }

  .console {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin: 0 -6px;
    padding: 10px 12px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.45);
    border: 1px solid var(--line);
    font: 450 11px/1.65 var(--mono);
    color: #b9bdc6;
    user-select: text;
  }
  .log {
    white-space: pre-wrap;
    word-break: break-all;
  }
  .log.warn {
    color: var(--stale);
  }
  .log.err {
    color: var(--hot);
  }
</style>
