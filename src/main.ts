import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import "./design/tokens.css";
import "./design/controls.css";
import App from "./App.svelte";

const scriptStart = performance.timeOrigin + performance.now();

const app = mount(App, { target: document.getElementById("app")! });

// Report the first painted frame (startup budget, scripts/bench-startup.mjs).
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    const nav = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
    const t0 = performance.timeOrigin;
    invoke("app_ready", {
      marks: {
        navStart: t0,
        scriptStart,
        domContentLoaded: nav ? t0 + nav.domContentLoadedEventStart : 0,
        firstFrame: t0 + performance.now(),
      },
    }).catch(() => {
      /* running in a plain browser (vite dev without Tauri) */
    });
  }),
);

export default app;
