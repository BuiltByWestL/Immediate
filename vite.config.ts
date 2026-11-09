import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { cpSync, existsSync, readFileSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import process from "node:process";

/**
 * pdf.js loads some files only when a PDF needs them: character maps (CJK text), the standard
 * 14 fonts (PDFs that don't embed them), JPEG 2000 / JBIG2 decoders and a color profile. Served
 * from node_modules while developing and copied into the build under /pdfjs/.
 */
function pdfjsAssets(): Plugin {
  const root = resolve("node_modules/pdfjs-dist");
  const dirs = ["cmaps", "standard_fonts", "wasm", "iccs"];
  // The JavaScript sandbox (scripting in PDFs) stays out: Immediate doesn't run PDF scripts.
  const wanted = (file: string) => !basename(file).startsWith("quickjs");
  return {
    name: "pdfjs-assets",
    configureServer(server) {
      server.middlewares.use("/pdfjs", (req, res, next) => {
        const file = join(root, decodeURIComponent((req.url ?? "").split("?")[0]));
        if (!file.startsWith(root) || !existsSync(file) || !statSync(file).isFile() || !wanted(file)) return next();
        res.setHeader("Content-Type", file.endsWith(".wasm") ? "application/wasm" : "application/octet-stream");
        res.end(readFileSync(file));
      });
    },
    writeBundle(options) {
      for (const d of dirs) cpSync(join(root, d), join(options.dir ?? "dist", "pdfjs", d), { recursive: true, filter: wanted });
    },
  };
}

const host = process.env.TAURI_DEV_HOST;

// Tauri expects a fixed port and handles its own console output.
export default defineConfig({
  plugins: [svelte(), pdfjsAssets()],
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
