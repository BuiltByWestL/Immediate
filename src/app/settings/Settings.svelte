<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import oflText from "../../fonts/OFL.txt?raw";
  import { ACCENTS, BUILTIN, GROUPS, customThemes, findTheme, resolveTheme, setCustomThemes, withAccent, type Theme } from "../../design/theme";
  import { inTauri, toast, ui, updateSettings } from "../../lib/app.svelte";
  import type { Settings } from "../../lib/types";
  import { checkForUpdates, updates } from "../../lib/updates.svelte";
  import { checkTools, installTools, tools } from "../../lib/tools.svelte";
  import Sheet from "../Sheet.svelte";
  import ShortcutList from "../ShortcutList.svelte";
  import ThemeCard from "./ThemeCard.svelte";

  const SECTIONS = [
    ["general", "General"],
    ["appearance", "Appearance"],
    ["tools", "Media tools"],
    ["shortcuts", "Shortcuts"],
    ["about", "About"],
  ] as const;

  const s = $derived(ui.settings);
  const set = (patch: Partial<Settings>) => updateSettings(patch);

  let custom = $state<Theme[]>(customThemes());
  let version = $state("");
  let showOfl = $state(false);
  let tokenPresent = $state(false);
  let tokenDraft = $state("");
  let tokenBusy = $state(false);

  async function saveToken(value: string) {
    tokenBusy = true;
    try {
      tokenPresent = await invoke<boolean>("update_token_set", { token: value });
      tokenDraft = "";
      if (tokenPresent) checkForUpdates(true);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      tokenBusy = false;
    }
  }
  let helveticaInstalled = $state(false);

  onMount(() => {
    helveticaInstalled = fontInstalled("Helvetica Neue");
    if (!inTauri) return;
    getVersion().then((v) => (version = v));
    invoke<boolean>("update_token_present").then((p) => (tokenPresent = p));
    checkTools();
    invoke<Theme[]>("themes_custom").then((list) => {
      setCustomThemes(list);
      custom = list;
    });
  });

  /** A font is installed if text set in it measures differently from the fallback. */
  function fontInstalled(family: string) {
    const ctx = document.createElement("canvas").getContext("2d");
    if (!ctx) return false;
    const sample = "abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789";
    return ["monospace", "serif"].some((fallback) => {
      ctx.font = `48px ${fallback}`;
      const a = ctx.measureText(sample).width;
      ctx.font = `48px "${family}", ${fallback}`;
      return ctx.measureText(sample).width !== a;
    });
  }

  const osDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const pair = (light: string, dark: string) => [findTheme(light), findTheme(dark)].filter(Boolean) as Theme[];
  const accentPreview = (t: Theme) => withAccent(t, s.accent);
  const currentScheme = $derived(resolveTheme(s).scheme);
  const lightThemes = $derived([...BUILTIN, ...custom].filter((t) => t.scheme === "light"));
  const darkThemes = $derived([...BUILTIN, ...custom].filter((t) => t.scheme === "dark"));

  async function openThemesFolder() {
    try {
      const dir = await invoke<string>("themes_folder");
      await invoke("reveal", { path: dir });
    } catch (e) {
      toast(String(e), "error");
    }
  }

  const suffixExample = $derived(`photo${s.saveSuffix}.jpg`);
</script>

<Sheet title="Settings" width={780} flush onclose={() => (ui.dialog = null)}>
  <div class="layout">
    <nav aria-label="Settings sections">
      {#each SECTIONS as [id, label] (id)}
        <button type="button" class:active={ui.settingsSection === id} aria-current={ui.settingsSection === id} onclick={() => (ui.settingsSection = id)}>
          {label}
        </button>
      {/each}
    </nav>

    <div class="content">
      {#if ui.settingsSection === "general"}
        <section>
          <h3>Mode</h3>
          <div class="segmented" role="radiogroup" aria-label="Mode">
            <button type="button" role="radio" aria-checked={s.mode === "simple"} onclick={() => set({ mode: "simple" })}>Simple</button>
            <button type="button" role="radio" aria-checked={s.mode === "advanced"} onclick={() => set({ mode: "advanced" })}>Advanced</button>
          </div>
          <p class="note">
            {#if s.mode === "simple"}
              Shows the tools most people need: crop, filters and the main sliders for photos; trim and cut for video and audio.
            {:else}
              Shows every tool and export option: all sliders, encoder and format settings, tracks, and metadata.
            {/if}
            You can switch at any time without losing edits.
          </p>
        </section>

        <section>
          <h3>Saving</h3>
          <p class="note first">Save never touches your original. It saves a copy next to it, in the same format and quality.</p>
          <label class="row">
            <span>Added to the copy’s name</span>
            <input class="field suffix" value={s.saveSuffix} maxlength="40" spellcheck="false" onchange={(e) => set({ saveSuffix: e.currentTarget.value })} />
          </label>
          <p class="note">For example: <span class="example">{suffixExample}</span></p>
          <div class="row">
            <span>Remove location from saved photos</span>
            <button type="button" class="switch" role="switch" aria-checked={s.removeLocation} aria-label="Remove location from saved photos" onclick={() => set({ removeLocation: !s.removeLocation })}></button>
          </div>
        </section>

        <section>
          <h3>Updates</h3>
          <div class="row">
            <span>Check for updates automatically</span>
            <button type="button" class="switch" role="switch" aria-checked={s.checkUpdates} aria-label="Check for updates automatically" onclick={() => set({ checkUpdates: !s.checkUpdates })}></button>
          </div>
          <div class="row">
            <span class="dim">
              {#if updates.phase === "checking"}
                Checking…
              {:else if updates.lastResult === "failed"}
                <span title={updates.error}>Couldn’t reach the update server. Try again later.</span>
              {:else if updates.lastResult === "available" && updates.available}
                Immediate {updates.available.version} is available.
              {:else if updates.lastResult === "current"}
                Immediate is up to date.
              {:else}
                You have Immediate <span class="num">{version}</span>.
              {/if}
            </span>
            <button type="button" class="btn" disabled={!inTauri || updates.phase === "checking"} onclick={() => checkForUpdates(true)}>Check now</button>
          </div>
          {#if s.mode === "advanced" || !tokenPresent}
            <div class="token">
              <p class="note first">
                While Immediate’s repository is private, updates need a read-only GitHub access token.
                <button type="button" class="link" onclick={() => openUrl("https://github.com/settings/personal-access-tokens/new")}>Create one</button>
                with access to the Immediate repository and the permission “Contents: Read-only”, then paste it here. It’s kept in {window.__IMMEDIATE__?.platform === "windows" ? "Windows Credential Manager" : window.__IMMEDIATE__?.platform === "macos" ? "the Keychain" : "a private file"}.
              </p>
              <div class="row">
                {#if tokenPresent}
                  <span class="dim">A token is saved.</span>
                  <button type="button" class="btn" disabled={tokenBusy} onclick={() => saveToken("")}>Remove</button>
                {:else}
                  <input class="field grow" type="password" placeholder="github_pat_…" bind:value={tokenDraft} spellcheck="false" autocomplete="off" />
                  <button type="button" class="btn" disabled={tokenBusy || !tokenDraft.trim()} onclick={() => saveToken(tokenDraft)}>Save</button>
                {/if}
              </div>
            </div>
          {/if}
        </section>
      {:else if ui.settingsSection === "appearance"}
        <section>
          <h3>Theme</h3>
          <div class="grid" role="radiogroup" aria-label="Theme">
            <ThemeCard
              name="System"
              themes={pair(s.systemLight, s.systemDark).map(accentPreview)}
              selected={s.theme === "system"}
              onselect={() => set({ theme: "system" })}
            />
          </div>
          {#if s.theme === "system"}
            <div class="pairing">
              <label>
                <span class="dim">Light</span>
                <select class="field" value={s.systemLight} onchange={(e) => set({ systemLight: e.currentTarget.value })}>
                  {#each lightThemes as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
                </select>
              </label>
              <label>
                <span class="dim">Dark</span>
                <select class="field" value={s.systemDark} onchange={(e) => set({ systemDark: e.currentTarget.value })}>
                  {#each darkThemes as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
                </select>
              </label>
              <span class="dim">Follows your {osDark ? "dark" : "light"} system setting.</span>
            </div>
          {/if}

          {#each GROUPS as g (g)}
            <h4>{g}</h4>
            <div class="grid" role="radiogroup" aria-label="{g} themes">
              {#each BUILTIN.filter((t) => t.group === g) as t (t.id)}
                <ThemeCard name={t.name} themes={[accentPreview(t)]} selected={s.theme === t.id} onselect={() => set({ theme: t.id })} />
              {/each}
            </div>
          {/each}

          <h4>Custom</h4>
          {#if custom.length}
            <div class="grid" role="radiogroup" aria-label="Custom themes">
              {#each custom as t (t.id)}
                <ThemeCard name={t.name} themes={[accentPreview(t)]} selected={s.theme === t.id} onselect={() => set({ theme: t.id })} />
              {/each}
            </div>
          {/if}
          <p class="note first">
            Make your own by adding a theme file to the themes folder; the README there shows how.
            <button type="button" class="link" onclick={openThemesFolder}>Open themes folder</button>
          </p>
        </section>

        <section>
          <h3>Accent color</h3>
          <div class="swatches" role="radiogroup" aria-label="Accent color">
            <button type="button" class="swatch theme-default" role="radio" aria-checked={s.accent === null} title="Theme’s own" aria-label="Theme’s own" onclick={() => set({ accent: null })}>
              <span>Theme</span>
            </button>
            {#each ACCENTS as a (a.id)}
              <button
                type="button"
                class="swatch"
                role="radio"
                aria-checked={s.accent === a.id}
                title={a.name}
                aria-label={a.name}
                style:--swatch={a[currentScheme].accent}
                onclick={() => set({ accent: a.id })}
              ></button>
            {/each}
          </div>
        </section>

        <section>
          <h3>Font</h3>
          <div class="choices" role="radiogroup" aria-label="Font">
            <button type="button" role="radio" class="choice" aria-checked={s.font === "immediate"} onclick={() => set({ font: "immediate" })}>
              <span class="dot"></span><span>Immediate Sans <span class="dim">(built in, the same on every computer)</span></span>
            </button>
            <button type="button" role="radio" class="choice" aria-checked={s.font === "helvetica"} disabled={!helveticaInstalled && s.font !== "helvetica"} onclick={() => set({ font: "helvetica" })}>
              <span class="dot"></span><span style:font-family="'Helvetica Neue', var(--font)">Helvetica Neue {#if !helveticaInstalled}<span class="dim">(not installed)</span>{/if}</span>
            </button>
            <button type="button" role="radio" class="choice" aria-checked={s.font === "system"} onclick={() => set({ font: "system" })}>
              <span class="dot"></span><span style:font-family="system-ui">System font</span>
            </button>
          </div>
        </section>

        <section>
          <h3>Display</h3>
          <div class="row">
            <span>Show other files in the folder along the bottom</span>
            <button type="button" class="switch" role="switch" aria-checked={s.filmstrip} aria-label="Show the filmstrip" onclick={() => set({ filmstrip: !s.filmstrip })}></button>
          </div>
          <div class="row">
            <span>Reduce motion</span>
            <button type="button" class="switch" role="switch" aria-checked={s.reduceMotion} aria-label="Reduce motion" onclick={() => set({ reduceMotion: !s.reduceMotion })}></button>
          </div>
          <div class="row">
            <span>Tint the background behind photos and video with the theme</span>
            <button type="button" class="switch" role="switch" aria-checked={s.tintCanvas} aria-label="Tint the background behind photos and video" onclick={() => set({ tintCanvas: !s.tintCanvas })}></button>
          </div>
          <p class="note">Off keeps it neutral grey, so a colorful theme never changes how a photo’s colors look to you.</p>
        </section>
      {:else if ui.settingsSection === "tools"}
        <section>
          <h3>Media tools</h3>
          <p class="note first">
            Video and audio editing uses ffmpeg, a free media toolkit that runs as a separate program. Immediate sets it up by itself when needed. Photos never need it.
          </p>
          {#if tools.found === undefined}
            <p class="dim">Looking…</p>
          {:else if tools.found}
            <dl class="facts">
              <dt>Version</dt>
              <dd class="num">{tools.found.version}</dd>
              <dt>Source</dt>
              <dd>{tools.found.managed ? "Installed by Immediate" : "Already on this computer"}</dd>
              <dt>Location</dt>
              <dd class="path dim">{tools.found.ffmpeg}</dd>
            </dl>
          {:else}
            <p class="dim">Not set up yet.</p>
          {/if}
          <div class="row">
            <span class="dim">
              {#if tools.installing}Setting up… {tools.total ? Math.round((tools.received / tools.total) * 100) + "%" : ""}{:else if tools.error}{tools.error}{/if}
            </span>
            <button type="button" class="btn" disabled={!inTauri || tools.installing} onclick={installTools}>
              {tools.found?.managed ? "Update" : tools.found ? "Use Immediate’s own copy" : "Set up now"}
            </button>
          </div>
        </section>
      {:else if ui.settingsSection === "shortcuts"}
        <section>
          <ShortcutList />
        </section>
      {:else}
        <section>
          <h3>Immediate <span class="num dim">{version}</span></h3>
          <p class="note first">Open any media file, make a quick edit, save a file. No accounts, no tracking, no project files.</p>
        </section>
        <section>
          <h3>Licenses</h3>
          <dl class="facts">
            <dt>Immediate</dt>
            <dd>MIT License</dd>
            <dt>Inter</dt>
            <dd>SIL Open Font License 1.1, by Rasmus Andersson. <button type="button" class="link" onclick={() => (showOfl = !showOfl)}>{showOfl ? "Hide" : "Show"} license</button></dd>
            <dt>Lucide icons</dt>
            <dd>ISC License</dd>
            <dt>ffmpeg</dt>
            <dd>Used as a separate program when installed; not part of Immediate.</dd>
          </dl>
          {#if showOfl}
            <pre class="license">{oflText}</pre>
          {/if}
        </section>
      {/if}
    </div>
  </div>
</Sheet>

<style>
  .layout {
    display: flex;
    height: min(600px, calc(100vh - 48px - 53px));
  }
  nav {
    flex: none;
    width: 168px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--s3);
    background: var(--panel);
    border-right: 1px solid var(--line);
  }
  nav button {
    height: var(--control-h);
    padding: 0 var(--s3);
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    text-align: left;
    color: var(--text-dim);
    font-weight: var(--w-medium);
    cursor: pointer;
  }
  nav button:hover {
    color: var(--text);
  }
  nav button.active {
    background: var(--hover);
    color: var(--text);
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: var(--s2) var(--s6) var(--s6);
  }
  section {
    padding: var(--s4) 0;
    border-bottom: 1px solid var(--line);
  }
  section:last-child {
    border-bottom: 0;
  }
  h3 {
    margin-bottom: var(--s3);
    font-size: var(--text-md);
    font-weight: var(--w-medium);
  }
  h4 {
    margin: var(--s5) 0 var(--s3);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
    color: var(--text-dim);
  }
  .note {
    margin-top: var(--s2);
    color: var(--text-dim);
    max-width: 62ch;
  }
  .note.first {
    margin: 0 0 var(--s3);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s4);
    min-height: 36px;
  }
  .token {
    margin-top: var(--s3);
  }
  .grow {
    flex: 1;
  }
  .suffix {
    width: 140px;
  }
  .example {
    color: var(--text);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: var(--s4) var(--s3);
  }
  .pairing {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s3) var(--s4);
    margin-top: var(--s3);
  }
  .pairing label {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s2);
  }
  .swatch {
    width: 28px;
    height: 28px;
    padding: 0;
    border-radius: 50%;
    border: 0;
    background: var(--swatch);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
    cursor: pointer;
  }
  .swatch[aria-checked="true"] {
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--focus);
  }
  .swatch.theme-default {
    width: auto;
    padding: 0 var(--s3);
    border-radius: 14px;
    background: var(--raised);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
  }
  .swatch.theme-default[aria-checked="true"] {
    box-shadow: inset 0 0 0 1px var(--line), 0 0 0 2px var(--bg), 0 0 0 4px var(--focus);
  }
  .choices {
    display: grid;
    gap: var(--s1);
  }
  .choice {
    display: flex;
    align-items: center;
    gap: var(--s3);
    min-height: 32px;
    padding: 0 var(--s2);
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .choice:hover {
    background: var(--hover);
  }
  .choice:disabled {
    cursor: default;
    background: transparent;
  }
  .dot {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
    background: var(--raised);
  }
  .choice[aria-checked="true"] .dot {
    border: 5px solid var(--accent);
    background: var(--on-accent);
  }
  .facts {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: var(--s2) var(--s4);
    margin: 0;
  }
  .facts dt {
    color: var(--text-dim);
  }
  .facts dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .path {
    display: block;
    font-size: var(--text-sm);
    user-select: text;
    -webkit-user-select: text;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .license {
    margin: var(--s3) 0 0;
    padding: var(--s3);
    max-height: 240px;
    overflow: auto;
    border-radius: var(--radius);
    background: var(--panel);
    font-size: var(--text-sm);
    white-space: pre-wrap;
    user-select: text;
    -webkit-user-select: text;
  }
</style>
