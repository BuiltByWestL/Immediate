# Immediate

Open any photo, video or audio file, make a quick edit, save a file. No projects, no import step.

Windows · macOS · Linux. Built with Tauri 2 (Rust) and Svelte 5.

## Develop

Requirements: Node 20+, Rust stable, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS (on Windows: VS Build Tools with C++ and WebView2, both already on Windows 11).

```sh
npm install
npm run tauri dev          # run the app with hot reload
npm run check              # type-check
npm run test:themes        # every theme passes the contrast check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build        # release build + installer
npm run bench:startup      # startup budget (after a release build)
```

## Scripts

| Script | What it does |
|---|---|
| `scripts/subset-fonts.py` | Builds `src/fonts/InterVariable-subset.woff2` from Inter's release (`pip install fonttools brotli`, then `python -I scripts/subset-fonts.py path/to/InterVariable.ttf`). |
| `scripts/font-specimen.html` | Side-by-side of the shipped Inter tuning vs. Helvetica Neue installed on your machine. Open in a browser. |
| `scripts/bench-startup.mjs` | Launches the release binary repeatedly and checks the startup budgets (window ≤ 100 ms, UI ≤ 400 ms). |
| `scripts/theme-contrast-test.mjs` | Checks every theme and accent for legible contrast (WCAG). Pass a folder to also check custom themes. |
| `scripts/gen-associations.mjs` | Regenerates the Windows installer's "Open with" / right-click registration and the macOS/Linux file associations from `src/formats.json`. Run after changing that file. |
| `scripts/make-fixtures.mjs` | Generates small test media in `test-fixtures/` with ffmpeg. |
| `scripts/playback-matrix.html` | Shows which fixtures the webview engine decodes natively (the rest use an ffmpeg preview proxy). |

## Themes

Themes are JSON files in `src/design/themes/` (13 color tokens each); accents are in `src/design/accents.json`. Users can add their own in `<config folder>/themes/` (Settings → Appearance → Open themes folder).

## File types

`src/formats.json` is the one list of supported file types, used by the app, the installer and the OS file associations.

## Fonts

The UI typeface is [Inter](https://github.com/rsms/inter) 4.1 (SIL Open Font License 1.1, see `src/fonts/OFL.txt`), tuned to read like Helvetica Neue: optical size 28, weights 420 (≈ Roman) and 570 (≈ Medium), `cv10` + `ss03`. Helvetica Neue itself is never committed or shipped.

## License

MIT. ffmpeg is downloaded separately on first use and run as an external program; it is not part of this repository or its installers.
