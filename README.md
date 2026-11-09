# Immediate

Open any photo, video or audio file, make a quick edit, save a file. No projects, no import step.

Windows · macOS · Linux. Built with Tauri 2 (Rust) and Svelte 5.

## What it does

- **Photos:** adjust (light, color, detail), filters, crop and straighten, markup (pen, pencil, arrows, shapes that snap neat, text, numbered steps, blur, a color picker), saved in the same format and quality with metadata kept, or as another format, size or file-size limit.
- **Video:** trim, split and cut out parts losslessly (or frame-exact), sound changes heard while playing, turn / mirror / crop, speed, pick or add tracks and subtitles, a thumbnail frame, make smaller for upload limits, save as GIF or animated WebP or save the sound only, change the format (MP4, MOV, MKV, WebM) without re-encoding where possible, optional graphics-card encoding.
- **Audio:** record (a microphone, or on Windows what the computer plays), trim, cut, silence, fades, volume, even out loudness (or peak), tone and a 10-band equalizer with low/high cut, noise reduction, leveling, speed with or without the pitch, reverse, mono/stereo, tags and cover art, spectrogram, save as WAV / FLAC / MP3 / M4A / Opus / OGG / AIFF at a chosen sample rate and bit depth.
- **Documents:** text and code, Markdown (with its pictures), CSV, PDF, spreadsheets and Word files.
- **Everywhere:** recent files, files open by their contents when the extension is missing, and an option to keep Immediate ready in the background so files open instantly.

## Install

Download the installer for your system from [Releases](https://github.com/BuiltByWestL/Immediate/releases). Updates install from inside the app.

- **Windows:** the installer isn't code-signed yet, so SmartScreen may say "Windows protected your PC": choose **More info → Run anyway**.
- **macOS:** the app isn't notarized (that needs a paid Apple account). The first time, open it, then go to **System Settings → Privacy & Security** and choose **Open Anyway**.
- **Linux:** `.deb` for Debian/Ubuntu, `.rpm` for Fedora/openSUSE, or the AppImage for anything else.

Video and audio editing use ffmpeg, which Immediate downloads by itself the first time it's needed (or uses the one already on your system).

## Develop

Requirements: Node 20+, Rust stable, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS (on Windows: VS Build Tools with C++ and WebView2, both already on Windows 11).

```sh
npm install
npm run tauri dev          # run the app with hot reload
npm run check              # type-check
npm run test:themes        # every theme passes the contrast check
npm run test:golden        # photo pipeline: preview = saved, no drift (uses installed Chrome/Edge)
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
| `scripts/golden-test.mjs` | Golden-image test of the photo pipeline (`--update` rewrites the references in `tests/golden/`). |
| `scripts/release-notes.mjs` | Prints a version's notes from `CHANGELOG.md`; the release workflow uses them for the release page and the in-app update offer. |
| `scripts/playback-matrix.html` | Shows which fixtures the webview engine decodes natively (the rest use an ffmpeg preview proxy). |

## Themes

Themes are JSON files in `src/design/themes/` (13 color tokens each); accents are in `src/design/accents.json`. Users can add their own in `<config folder>/themes/` (Settings → Appearance → Open themes folder).

## File types

`src/formats.json` is the one list of supported file types, used by the app, the installer and the OS file associations.

## Fonts

The UI typeface is [Inter](https://github.com/rsms/inter) 4.1 (SIL Open Font License 1.1, see `src/fonts/OFL.txt`), tuned to read like Helvetica Neue: optical size 28, weights 420 (≈ Roman) and 570 (≈ Medium), `cv10` + `ss03`. Helvetica Neue itself is never committed or shipped.

## License

MIT. ffmpeg is downloaded separately on first use and run as an external program; it is not part of this repository or its installers.
