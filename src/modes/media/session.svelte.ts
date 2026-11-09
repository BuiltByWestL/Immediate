// Video/audio editing session. The edit is: trim (keep start..end) minus cut-out ranges, all in
// source time. Nothing touches the file until Save.

import { ui } from "../../lib/app.svelte";

export interface Range {
  start: number;
  end: number;
}

export interface MediaEdit {
  trim: Range;
  cuts: Range[];
  /** Split marks (source seconds). They change nothing by themselves; double-clicking
   *  between two of them (or a split and an end) picks that part, ready to cut out. */
  splits: number[];
  mute: boolean;
  /** Volume change in dB. */
  gain: number;
  /** Fade lengths in seconds (0 = none). */
  fadeIn: number;
  fadeOut: number;
  normalize: boolean;
  /** Bass, mid, treble in dB. */
  eq: [number, number, number];
  /** Graphic equalizer, dB per band (GEQ_BANDS). */
  geq: number[];
  /** Low cut (high-pass) and high cut (low-pass) in Hz; 0 = off. */
  lowCut: number;
  highCut: number;
  /** "Even out loudness" by the loudest peak (-1 dB) instead of loudness (-16 LUFS). */
  normalizePeak: boolean;
  /** Audio files: the result plays backwards. */
  reverse: boolean;
  /** Audio files: 1 (mono), 2 (stereo), null = as the original. */
  channels: 1 | 2 | null;
  /** Parts made silent (source seconds). */
  silences: Range[];
  denoise: boolean;
  compress: boolean;
  /** Picture: clockwise turn (0/90/180/270), mirror, and a crop of the turned picture as
   *  fractions of its width and height (null = whole frame). */
  rotate: number;
  flip: boolean;
  crop: Range2D | null;
  /** Playback speed of the result (1 = unchanged), keeping the pitch or letting it follow. */
  speed: number;
  keepPitch: boolean;
  /** Tracks to keep, by their order in the file (missing = keep). */
  keepAudio: boolean[];
  keepSubs: boolean[];
  /** A subtitle file to add as a track. */
  subFile: string | null;
  /** Video: the frame (source seconds) saved as the file's thumbnail, or null to leave it. */
  coverFrame: number | null;
  /** Audio files: changed tags (only the ones that differ from the file), the cover art (null
   *  keeps it, "" removes it, a path replaces it), and a new sample rate / bit depth. */
  tags: Record<string, string>;
  cover: string | null;
  sampleRate: number | null;
  bitDepth: number | null;
}

export interface Range2D {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Graphic equalizer bands (Hz); export.rs uses the same (GEQ_HZ), each a peak at Q 1.41. */
export const GEQ_BANDS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

export function blankEdit(duration: number): MediaEdit {
  return {
    trim: { start: 0, end: duration },
    cuts: [],
    splits: [],
    mute: false,
    gain: 0,
    fadeIn: 0,
    fadeOut: 0,
    normalize: false,
    eq: [0, 0, 0],
    geq: GEQ_BANDS.map(() => 0),
    lowCut: 0,
    highCut: 0,
    normalizePeak: false,
    reverse: false,
    channels: null,
    silences: [],
    denoise: false,
    compress: false,
    rotate: 0,
    flip: false,
    crop: null,
    speed: 1,
    keepPitch: true,
    keepAudio: [],
    keepSubs: [],
    subFile: null,
    coverFrame: null,
    tags: {},
    cover: null,
    sampleRate: null,
    bitDepth: null,
  };
}

export type ExportMode = "lossless" | "accurate" | "smaller";

export const media = $state({
  active: false,
  /** ffprobe JSON of the source. */
  probe: null as any,
  duration: 0,
  keyframes: [] as number[],
  edit: blankEdit(0),
  past: [] as MediaEdit[],
  future: [] as MediaEdit[],
  /** Current play position, source seconds. */
  time: 0,
  playing: false,
  /** A range picked on the timeline, to cut out (or null). */
  selection: null as Range | null,
  mode: "lossless" as ExportMode,
  sizeMb: 10,
  saving: false,
  progress: 0,
  eta: null as number | null,
  /** Visible part of the timeline (seconds), for zoom. */
  view: { start: 0, end: 0 } as Range,
  /** The crop's shape while cropping: width / height in pixels, or null for any shape. */
  cropAspect: null as number | null,
  /** Loudest sample of the file (0–1), from the waveform; for the clipping warning. */
  peak: 0,
  /** The file's own cover art (an object URL), if it has one. */
  coverUrl: null as string | null,
});

let committed = "";

export function startMediaEdit(probe: any, duration: number) {
  media.probe = probe;
  media.duration = duration;
  media.edit = blankEdit(duration);
  media.cropAspect = null;
  media.past = [];
  media.future = [];
  media.selection = null;
  // Settings → Video → how video saves by default.
  media.mode = ui.settings.videoMode === "accurate" ? "accurate" : "lossless";
  media.view = { start: 0, end: duration };
  media.active = true;
  committed = JSON.stringify(media.edit);
}

export function stopMediaEdit() {
  media.active = false;
  media.playing = false;
}

/** Records the current edit as one undo step. */
export function commitMedia() {
  const now = JSON.stringify(media.edit);
  if (now === committed) return;
  media.past = [...media.past.slice(-99), JSON.parse(committed)];
  media.future = [];
  committed = now;
}

export function changeMedia(fn: (e: MediaEdit) => void) {
  fn(media.edit);
  commitMedia();
}

export function undoMedia() {
  const prev = media.past.at(-1);
  if (!prev) return;
  media.past = media.past.slice(0, -1);
  media.future = [JSON.parse(committed), ...media.future];
  media.edit = prev;
  committed = JSON.stringify(prev);
}

export function redoMedia() {
  const next = media.future[0];
  if (!next) return;
  media.future = media.future.slice(1);
  media.past = [...media.past, JSON.parse(committed)];
  media.edit = next;
  committed = JSON.stringify(next);
}

export function mediaChanged(): boolean {
  const e = media.edit;
  return (
    media.active &&
    (e.trim.start > 0.001 ||
      e.trim.end < media.duration - 0.001 ||
      e.cuts.length > 0 ||
      e.mute ||
      Math.abs(e.gain) > 0.01 ||
      e.fadeIn > 0 ||
      e.fadeOut > 0 ||
      e.normalize ||
      e.eq.some((g) => g !== 0) ||
      e.geq.some((g) => g !== 0) ||
      e.lowCut > 0 ||
      e.highCut > 0 ||
      e.reverse ||
      e.channels !== null ||
      e.silences.length > 0 ||
      e.denoise ||
      e.compress ||
      pictureChanged(e) ||
      e.speed !== 1 ||
      e.keepAudio.includes(false) ||
      e.keepSubs.includes(false) ||
      !!e.subFile ||
      e.coverFrame !== null ||
      Object.keys(e.tags).length > 0 ||
      e.cover !== null ||
      e.sampleRate !== null ||
      e.bitDepth !== null)
  );
}

/** Turned, mirrored or cropped. */
export function pictureChanged(e: MediaEdit = media.edit): boolean {
  return e.rotate % 360 !== 0 || e.flip || !!(e.crop && (e.crop.w < 0.999 || e.crop.h < 0.999));
}

/** Merged, sorted cut ranges inside the trim. */
function normalizedCuts(e: MediaEdit): Range[] {
  const cuts = e.cuts
    .map((c) => ({ start: Math.max(c.start, e.trim.start), end: Math.min(c.end, e.trim.end) }))
    .filter((c) => c.end > c.start)
    .sort((a, b) => a.start - b.start);
  const out: Range[] = [];
  for (const c of cuts) {
    const last = out.at(-1);
    if (last && c.start <= last.end) last.end = Math.max(last.end, c.end);
    else out.push({ ...c });
  }
  return out;
}

/** The parts that are kept, in order. */
export function keptSegments(e: MediaEdit = media.edit): Range[] {
  const out: Range[] = [];
  let at = e.trim.start;
  for (const c of normalizedCuts(e)) {
    if (c.start > at) out.push({ start: at, end: c.start });
    at = Math.max(at, c.end);
  }
  if (e.trim.end > at) out.push({ start: at, end: e.trim.end });
  return out.filter((s) => s.end - s.start > 0.01);
}

/** Split marks that still sit inside a kept part. */
export function liveSplits(e: MediaEdit = media.edit): number[] {
  const kept = keptSegments(e);
  return [...new Set(e.splits)].filter((t) => kept.some((s) => t > s.start + 0.01 && t < s.end - 0.01)).sort((a, b) => a - b);
}

/** Splits at `t`. False when `t` isn't inside a kept part (or a split is already there). */
export function splitAt(t: number): boolean {
  const kept = keptSegments();
  if (!kept.some((s) => t > s.start + 0.05 && t < s.end - 0.05)) return false;
  if (media.edit.splits.some((x) => Math.abs(x - t) < 0.05)) return false;
  changeMedia((e) => (e.splits = [...liveSplits(e), t].sort((a, b) => a - b)));
  return true;
}

/** The part around `t`: between split marks (or a split and the end of its kept part), or the
 *  whole kept part when no split is inside it. Null in a removed part. */
export function partAt(t: number): Range | null {
  const part = keptSegments().find((s) => t >= s.start && t < s.end);
  if (!part) return null;
  const splits = liveSplits().filter((x) => x > part.start && x < part.end);
  const edges = [part.start, ...splits, part.end];
  for (let i = 0; i < edges.length - 1; i++) if (t >= edges[i] && t < edges[i + 1]) return { start: edges[i], end: edges[i + 1] };
  return null;
}

/** Makes the selection silent (audio). */
export function silenceSelection() {
  const s = media.selection;
  if (!s) return;
  const part = { ...s };
  changeMedia((e) => e.silences.push(part));
  media.selection = null;
}

/** Is source time `t` in a silenced part? */
export const silencedAt = (t: number, e: MediaEdit = media.edit) => e.silences.some((s) => t >= s.start && t < s.end);

/** The sound settings saving and measuring share (export.rs `Sound`). */
export function soundOf(e: MediaEdit = media.edit) {
  return { gainDb: e.gain, eq: [...e.eq], geq: [...e.geq], lowCut: e.lowCut, highCut: e.highCut, denoise: e.denoise, compress: e.compress, silences: e.silences.map((s) => ({ ...s })) };
}

/** Cuts out the selection. Returns where to put the playhead afterwards. */
export function cutSelection(): number | null {
  const s = media.selection;
  if (!s) return null;
  const cut = { ...s };
  changeMedia((e) => e.cuts.push(cut));
  media.selection = null;
  return cut.end;
}

/** Where lossless cuts really land: each kept part starts at the keyframe at or before it. */
export function snappedSegments(segs: Range[], keyframes: number[]): Range[] {
  if (!keyframes.length) return segs;
  return segs.map((s) => {
    let k = 0;
    for (const t of keyframes) {
      if (t <= s.start + 0.0005) k = t;
      else break;
    }
    return { start: k, end: s.end };
  });
}

export const totalLength = (segs: Range[]) => segs.reduce((n, s) => n + (s.end - s.start), 0);

/** If `t` is in a removed part, the next kept time (or null at the end). */
export function nextKept(t: number, segs: Range[]): number | null {
  for (const s of segs) {
    if (t < s.start - 0.001) return s.start;
    if (t < s.end - 0.01) return t;
  }
  return null;
}

export function fmtTime(t: number, precise = false): string {
  if (!isFinite(t)) return "0:00";
  const sign = t < 0 ? "-" : "";
  t = Math.abs(t);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  const ss = precise ? s.toFixed(1).padStart(4, "0") : String(Math.floor(s)).padStart(2, "0");
  return h ? `${sign}${h}:${String(m).padStart(2, "0")}:${ss}` : `${sign}${m}:${ss}`;
}
