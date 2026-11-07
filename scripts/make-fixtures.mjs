// Generate small synthetic media fixtures for playback / trim / export tests.
// Requires ffmpeg on PATH. Output: test-fixtures/ (git-ignored, regenerate any time).
//
// Usage: node scripts/make-fixtures.mjs

import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const OUT = resolve("test-fixtures");
mkdirSync(OUT, { recursive: true });

// 4 s of moving test pattern with a burnt-in frame counter + a 440/660 Hz tone.
const V = ["-f", "lavfi", "-i", "testsrc2=size=640x360:rate=30:duration=4"];
const A = ["-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000:duration=4"];
const A2 = ["-f", "lavfi", "-i", "sine=frequency=660:sample_rate=48000:duration=4"];
const GOP = ["-g", "30"]; // one keyframe per second, so keyframe snapping is visible

const fixtures = [
  ["h264-aac.mp4", [...V, ...A, "-c:v", "libx264", "-pix_fmt", "yuv420p", ...GOP, "-c:a", "aac", "-b:a", "128k"]],
  ["hevc-aac.mp4", [...V, ...A, "-c:v", "libx265", "-pix_fmt", "yuv420p", "-tag:v", "hvc1", "-x265-params", "keyint=30:log-level=error", "-c:a", "aac"]],
  ["hevc-10bit.mkv", [...V, ...A, "-c:v", "libx265", "-pix_fmt", "yuv420p10le", "-x265-params", "keyint=30:log-level=error", "-c:a", "aac"]],
  ["vp9-opus.webm", [...V, ...A, "-c:v", "libvpx-vp9", "-b:v", "500k", ...GOP, "-c:a", "libopus"]],
  ["av1-opus.mkv", [...V, ...A, "-c:v", "libsvtav1", "-preset", "10", ...GOP, "-c:a", "libopus"]],
  ["h264-ac3.mkv", [...V, ...A, "-c:v", "libx264", "-pix_fmt", "yuv420p", ...GOP, "-c:a", "ac3"]],
  ["prores-pcm.mov", [...V, ...A, "-c:v", "prores_ks", "-profile:v", "2", "-c:a", "pcm_s16le"]],
  ["mpeg2-mp2.mpg", [...V, ...A, "-c:v", "mpeg2video", "-b:v", "2M", ...GOP, "-c:a", "mp2"]],
  ["mpeg4-mp3.avi", [...V, ...A, "-c:v", "mpeg4", "-q:v", "5", ...GOP, "-c:a", "libmp3lame"]],
  [
    "two-audio-tracks-subs.mkv",
    [
      ...V, ...A, ...A2,
      "-map", "0:v", "-map", "1:a", "-map", "2:a",
      "-c:v", "libx264", "-pix_fmt", "yuv420p", ...GOP, "-c:a", "aac",
      "-metadata:s:a:0", "language=eng", "-metadata:s:a:1", "language=fra",
    ],
  ],
  ["tone.wav", [...A, "-c:a", "pcm_s24le"]],
  ["tone.flac", [...A, "-c:a", "flac"]],
  ["tone.mp3", [...A, "-c:a", "libmp3lame", "-b:a", "192k"]],
  ["tone.m4a", [...A, "-c:a", "aac", "-b:a", "160k"]],
  ["tone.opus", [...A, "-c:a", "libopus", "-b:a", "96k"]],
  ["still.jpg", ["-f", "lavfi", "-i", "testsrc2=size=1920x1080", "-frames:v", "1", "-q:v", "3"]],
  ["still.png", ["-f", "lavfi", "-i", "testsrc2=size=1920x1080", "-frames:v", "1"]],
];

let failed = 0;
for (const [name, args] of fixtures) {
  const r = spawnSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args, join(OUT, name)], {
    encoding: "utf8",
  });
  if (r.status === 0) {
    console.log(`ok    ${name}`);
  } else {
    failed++;
    console.log(`FAIL  ${name}\n      ${(r.stderr || r.error?.message || "").trim().split("\n").pop()}`);
  }
}

// Subtitles are muxed in a second pass (lavfi can't generate them).
const srt = join(OUT, "subs.srt");
import("node:fs").then(({ writeFileSync }) => {
  writeFileSync(srt, "1\n00:00:00,500 --> 00:00:02,000\nHello\n\n2\n00:00:02,500 --> 00:00:03,500\nWorld\n");
  const r = spawnSync("ffmpeg", [
    "-hide_banner", "-loglevel", "error", "-y",
    "-i", join(OUT, "two-audio-tracks-subs.mkv"), "-i", srt,
    "-map", "0", "-map", "1", "-c", "copy", "-c:s", "srt",
    join(OUT, "with-subs.mkv"),
  ]);
  console.log(r.status === 0 ? "ok    with-subs.mkv" : "FAIL  with-subs.mkv");
  if (failed || r.status !== 0) process.exitCode = 1;
});
