import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import "./design/tokens.css";
import App from "./App.svelte";

const scriptStart = performance.timeOrigin + performance.now();

const app = mount(App, { target: document.getElementById("app")! });

// The window is created hidden. Show it only after the first frame has been painted,
// so the user never sees an empty or half-styled window.
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
