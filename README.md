# Immediate

Open any photo, video or audio file, make a quick edit, save a file. No projects, no import step.

Windows · macOS · Linux. Built with Tauri 2 (Rust) and Svelte 5.

## Develop

Requirements: Node 20+, Rust stable, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS (on Windows: VS Build Tools with C++ and WebView2, both already on Windows 11).

```sh
npm install
npm run tauri dev          # run the app with hot reload
npm run check              # type-check
npm run tauri build        # release build + installer
npm run bench:startup      # startup budget (after a release build)
```

## Scripts

| Script | What it does |
|---|---|
| `scripts/subset-fonts.py` | Builds `src/fonts/InterVariable-subset.woff2` from Inter's release (`pip install fonttools brotli`, then `python -I scripts/subset-fonts.py path/to/InterVariable.ttf`). |
| `scripts/font-specimen.html` | Side-by-side of the shipped Inter tuning vs. Helvetica Neue installed on your machine. Open in a browser. |
| `scripts/bench-startup.mjs` | Launches the release binary repeatedly and checks the ≤ 300 ms startup budget. |
| `scripts/make-fixtures.mjs` | Generates small test media in `test-fixtures/` with ffmpeg. |
| `scripts/playback-matrix.html` | Shows which fixtures the webview engine decodes natively (the rest use an ffmpeg preview proxy). |

## Fonts

The UI typeface is [Inter](https://github.com/rsms/inter) 4.1 (SIL Open Font License 1.1, see `src/fonts/OFL.txt`), tuned to read like Helvetica Neue: optical size 28, weights 420 (≈ Roman) and 570 (≈ Medium), `cv10` + `ss03`. Helvetica Neue itself is never committed or shipped.

## License

MIT. ffmpeg is downloaded separately on first use and run as an external program; it is not part of this repository or its installers.
