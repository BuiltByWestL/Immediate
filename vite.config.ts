import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

// Tauri expects a fixed port and handles its own console output.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // Every webview Immediate targets (WebView2, WKWebView on macOS 13+, WebKitGTK 4.1) handles ES2022.
    target: "es2022",
    cssMinify: true,
    reportCompressedSize: false,
  },
});
