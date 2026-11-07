// Video/audio editing session. The edit is: trim (keep start..end) minus cut-out ranges, all in
// source time. Nothing touches the file until Save.

export interface Range {
  start: number;
  end: number;
}

export interface MediaEdit {
  trim: Range;
  cuts: Range[];
  mute: boolean;
  /** Volume change in dB. */
  gain: number;
}

export type ExportMode = "lossless" | "accurate" | "smaller";

export const media = $state({
  active: false,
  /** ffprobe JSON of the source. */
  probe: null as any,
  duration: 0,
  keyframes: [] as number[],
  edit: { trim: { start: 0, end: 0 }, cuts: [], mute: false, gain: 0 } as MediaEdit,
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
});

let committed = "";

export function startMediaEdit(probe: any, duration: number) {
  media.probe = probe;
  media.duration = duration;
  media.edit = { trim: { start: 0, end: duration }, cuts: [], mute: false, gain: 0 };
  media.past = [];
  media.future = [];
  media.selection = null;
  media.mode = "lossless";
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
  return media.active && (e.trim.start > 0.001 || e.trim.end < media.duration - 0.001 || e.cuts.length > 0 || e.mute || Math.abs(e.gain) > 0.01);
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
