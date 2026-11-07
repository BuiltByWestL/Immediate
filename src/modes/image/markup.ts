// Markup: shapes drawn on top of the photo. Points are stored in source pixels (the original,
// upright photo), so marks stay on the same spot of the photo through later crops and turns.
// One painter draws them for the live preview and for the saved file.

export type Pt = [number, number];

export type ToolId = "pen" | "highlighter" | "arrow" | "text" | "blur" | "line" | "rect" | "ellipse" | "number" | "eraser";

export interface Shape {
  kind: Exclude<ToolId, "eraser">;
  /** pen/highlighter: the stroke; line/arrow: [a, b]; rect/ellipse/blur: 4 corners; text/number: [at]. */
  pts: Pt[];
  color: string;
  /** Line width (or text size) in source pixels. */
  size: number;
  text?: string;
  n?: number;
}

export const COLORS = ["#ff3b30", "#ff9500", "#ffcc00", "#34c759", "#0a84ff", "#ffffff", "#111111"];

/** Line widths as a fraction of the photo's long side. */
export const SIZES = { s: 0.0025, m: 0.005, l: 0.01 } as const;

function contrastColor(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const l = 0.2126 * ((n >> 16) & 255) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255);
  return l > 150 ? "#111111" : "#ffffff";
}

const FONT = '"Immediate Sans", "Helvetica Neue", Arial, sans-serif';

/**
 * Paints shapes into `ctx`. `map` turns source pixels into canvas pixels; `k` is canvas pixels
 * per source pixel. Redaction reads the pixels already in `ctx.canvas` (or `under`, if given).
 */
export function paintMarkup(
  ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D,
  shapes: Shape[],
  map: (p: Pt) => Pt,
  k: number,
  under?: CanvasImageSource,
) {
  for (const s of shapes) {
    const pts = s.pts.map(map);
    const w = Math.max(0.5, s.size * k);
    ctx.save();
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    ctx.strokeStyle = s.color;
    ctx.fillStyle = s.color;
    ctx.lineWidth = w;
    switch (s.kind) {
      case "pen":
      case "highlighter": {
        if (s.kind === "highlighter") {
          ctx.globalAlpha = 0.4;
          ctx.globalCompositeOperation = "multiply";
          ctx.lineWidth = w * 3.5;
          ctx.lineCap = "square";
        }
        stroke(ctx, pts);
        break;
      }
      case "line":
      case "arrow": {
        const [a, b] = pts;
        ctx.beginPath();
        ctx.moveTo(a[0], a[1]);
        ctx.lineTo(b[0], b[1]);
        ctx.stroke();
        if (s.kind === "arrow") {
          const ang = Math.atan2(b[1] - a[1], b[0] - a[0]);
          const len = Math.max(w * 4.5, 8);
          ctx.beginPath();
          ctx.moveTo(b[0] - len * Math.cos(ang - 0.45), b[1] - len * Math.sin(ang - 0.45));
          ctx.lineTo(b[0], b[1]);
          ctx.lineTo(b[0] - len * Math.cos(ang + 0.45), b[1] - len * Math.sin(ang + 0.45));
          ctx.stroke();
        }
        break;
      }
      case "rect": {
        polygon(ctx, pts);
        ctx.stroke();
        break;
      }
      case "ellipse": {
        const [a, b, , d] = pts;
        const cx = (pts[0][0] + pts[2][0]) / 2;
        const cy = (pts[0][1] + pts[2][1]) / 2;
        const rx = Math.hypot(b[0] - a[0], b[1] - a[1]) / 2;
        const ry = Math.hypot(d[0] - a[0], d[1] - a[1]) / 2;
        ctx.beginPath();
        ctx.ellipse(cx, cy, rx, ry, Math.atan2(b[1] - a[1], b[0] - a[0]), 0, Math.PI * 2);
        ctx.stroke();
        break;
      }
      case "blur": {
        pixelate(ctx, pts, under ?? ctx.canvas);
        break;
      }
      case "text": {
        const size = s.size * k;
        ctx.font = `600 ${size}px ${FONT}`;
        ctx.textBaseline = "top";
        ctx.lineWidth = Math.max(1, size * 0.14);
        ctx.strokeStyle = contrastColor(s.color);
        const [x, y] = pts[0];
        (s.text ?? "").split("\n").forEach((line, i) => {
          ctx.strokeText(line, x, y + i * size * 1.2);
          ctx.fillText(line, x, y + i * size * 1.2);
        });
        break;
      }
      case "number": {
        const r = Math.max(6, w * 2.6);
        const [x, y] = pts[0];
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
        ctx.lineWidth = Math.max(1, r * 0.12);
        ctx.strokeStyle = contrastColor(s.color);
        ctx.stroke();
        ctx.fillStyle = contrastColor(s.color);
        ctx.font = `700 ${r * 1.15}px ${FONT}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(String(s.n ?? 1), x, y + r * 0.05);
        break;
      }
    }
    ctx.restore();
  }
}

function stroke(ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D, p: Pt[]) {
  if (!p.length) return;
  ctx.beginPath();
  ctx.moveTo(p[0][0], p[0][1]);
  if (p.length === 1) ctx.lineTo(p[0][0] + 0.01, p[0][1]);
  // Smooth through the midpoints so freehand lines don't look jagged.
  for (let i = 1; i < p.length - 1; i++) {
    const mx = (p[i][0] + p[i + 1][0]) / 2;
    const my = (p[i][1] + p[i + 1][1]) / 2;
    ctx.quadraticCurveTo(p[i][0], p[i][1], mx, my);
  }
  if (p.length > 1) ctx.lineTo(p[p.length - 1][0], p[p.length - 1][1]);
  ctx.stroke();
}

function polygon(ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D, p: Pt[]) {
  ctx.beginPath();
  p.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
  ctx.closePath();
}

/** Covers an area with big blocks (pixelation), so the hidden detail can't be recovered. */
function pixelate(ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D, p: Pt[], under: CanvasImageSource) {
  const xs = p.map((q) => q[0]);
  const ys = p.map((q) => q[1]);
  const x0 = Math.floor(Math.min(...xs));
  const y0 = Math.floor(Math.min(...ys));
  const w = Math.ceil(Math.max(...xs)) - x0;
  const h = Math.ceil(Math.max(...ys)) - y0;
  if (w < 1 || h < 1) return;
  const block = Math.max(6, Math.min(w, h) / 10);
  const tw = Math.max(1, Math.round(w / block));
  const th = Math.max(1, Math.round(h / block));
  const tmp = new OffscreenCanvas(tw, th);
  const t = tmp.getContext("2d")!;
  t.imageSmoothingEnabled = true;
  t.imageSmoothingQuality = "high";
  t.drawImage(under, x0, y0, w, h, 0, 0, tw, th);
  ctx.save();
  polygon(ctx, p);
  ctx.clip();
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(tmp, 0, 0, tw, th, x0, y0, w, h);
  ctx.restore();
}

/** The topmost shape near a point (source pixels), for the eraser. */
export function hitTest(shapes: Shape[], p: Pt, tolerance: number): number {
  const d2seg = (a: Pt, b: Pt) => {
    const dx = b[0] - a[0];
    const dy = b[1] - a[1];
    const t = Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy || 1)));
    return Math.hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy));
  };
  const inside = (q: Pt[]) => {
    let c = false;
    for (let i = 0, j = q.length - 1; i < q.length; j = i++) {
      if (q[i][1] > p[1] !== q[j][1] > p[1] && p[0] < ((q[j][0] - q[i][0]) * (p[1] - q[i][1])) / (q[j][1] - q[i][1]) + q[i][0]) c = !c;
    }
    return c;
  };
  for (let i = shapes.length - 1; i >= 0; i--) {
    const s = shapes[i];
    const tol = tolerance + s.size;
    const pts = s.pts;
    switch (s.kind) {
      case "text":
      case "number": {
        const size = s.kind === "text" ? s.size : s.size * 2.6;
        const wGuess = s.kind === "text" ? size * 0.6 * Math.max(...(s.text ?? "").split("\n").map((l) => l.length)) : size;
        const [x, y] = pts[0];
        const box: Pt[] = s.kind === "text"
          ? [[x, y], [x + wGuess, y], [x + wGuess, y + size * 1.2 * (s.text ?? "").split("\n").length], [x, y + size * 1.2]]
          : [[x - size, y - size], [x + size, y - size], [x + size, y + size], [x - size, y + size]];
        if (inside(box)) return i;
        break;
      }
      case "blur":
      case "rect":
      case "ellipse":
        if (inside(pts) || pts.some((a, j) => d2seg(a, pts[(j + 1) % pts.length]) < tol)) return i;
        break;
      default:
        for (let j = 0; j < pts.length; j++) if (d2seg(pts[j], pts[Math.min(j + 1, pts.length - 1)]) < tol) return i;
    }
  }
  return -1;
}
