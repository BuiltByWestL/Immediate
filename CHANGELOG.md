# What's new

Each release's notes come from its section here (the release page and the update offer in the app show them). Write them for people using Immediate, not for developers.

## 0.3.0

**Video**
- Split at the playhead: press W with nothing selected. Double-click a part between two splits to select it, then W cuts it out.
- Save as animated WebP (smoother and more colorful than a GIF).
- Pick any frame as the video's thumbnail (Advanced → Picture).
- Save a subtitle track as an .srt file, timed to your edit.
- Optional: re-encode with your graphics card (Settings → Video), several times faster.
- Settings → Video: choose whether video saves Lossless or Frame-exact by default.
- Fixed: cover art in MKV files no longer turns into extra video tracks when saving; MP4 cover art survives frame-exact saves.

**Audio**
- A 10-band equalizer with presets, plus low cut (rumble) and high cut (hiss).
- Silence a selected part, keeping its length.
- Even out loudness by the loudest peak, as well as by loudness.
- Play backwards; save as mono or stereo; save as OGG or AIFF.
- A warning when the volume would make the loudest parts distort.
- The recorder can count 3, 2, 1 before it starts, and remembers your microphone.
- Settings → Audio: save every audio file in one format (for example, recordings as MP3).
- Fixed: bass and treble now sound the same in the saved file as while editing (they were about 1 dB stronger when saved).

**Photos**
- Pencil, a color picker that takes a color from the photo, and hold-to-snap: pause at the end of a rough line, box or circle and it becomes a neat one.
- Advanced → Save: format, quality, size, "keep the file under" a size (for upload limits), and remove all details.
- HDR photos (iPhone, Pixel) say that saving keeps the picture but not the extra HDR brightness.

**Everywhere**
- Recent files on the start screen.
- Keep Immediate ready in the background (Settings → General) so files open instantly.
- Files with a missing or wrong extension open by what they contain.
- Updates can be installed from Settings, and Immediate asks before an update would close windows with unsaved work.
- PDFs with Chinese, Japanese or Korean text, or without embedded fonts, display properly; Markdown files show their pictures.
- macOS: the toolbar is the title bar. Linux: an .rpm package as well.

## 0.2.0

- Video: turn, mirror, crop and change speed; pick and add tracks and subtitles; save as GIF or just the sound; save as MP4, MOV, MKV or WebM.
- Audio: tags and cover art, sample rate and bit depth, spectrogram, and recording what the computer plays (Windows).
- Even out loudness now matches what you hear while editing.
- Windows reopen where you left them. Builds for macOS (Apple Silicon and Intel) and Linux.
