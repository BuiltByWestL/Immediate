// The photo pipeline: one WebGL2 fragment shader applies geometry (crop, rotate, flip,
// straighten, perspective) and every adjustment. The same code draws the live preview and
// renders the full-resolution export, so what you see is what you save.

import type { Adjust, Edits, Geometry } from "./edits";
import { effectiveAdjust } from "./edits";

const VERT = `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`;

// Indices into uA (keep in sync with ADJUST_ORDER below).
const ADJUST_ORDER: (keyof Adjust)[] = [
  "exposure", "brilliance", "highlights", "shadows", "contrast", "brightness", "blackPoint",
  "saturation", "vibrance", "warmth", "tint", "sharpness", "definition", "noise", "vignette",
  "bw", "bwNeutrals", "bwTone", "grain",
];

const FRAG = `#version 300 es
precision highp float;
uniform sampler2D uSrc;
uniform vec2 uSrcSize;      // source size, pixels
uniform float uLocalLod;    // mip level used as the "local mean" (≈ 1/40 of the image)
uniform vec2 uTarget;       // render target size, device pixels
uniform vec2 uViewOffset;   // where output pixel (0,0) lands in the target
uniform float uViewScale;   // target pixels per output pixel
uniform vec2 uOutSize;      // output (cropped) image size, pixels
uniform vec4 uCrop;         // crop in rotated space, fractions (x, y, w, h)
uniform vec2 uRotSize;      // rotated (quarter-turned) image size, pixels
uniform float uAngle;       // straighten, radians
uniform int uQuarter;       // quarter turns clockwise
uniform vec2 uFlip;         // 1 or -1 per axis
uniform vec2 uPers;         // perspective (horizontal, vertical), -1..1
uniform float uA[${ADJUST_ORDER.length}];
out vec4 outColor;

const vec3 W = vec3(0.2126, 0.7152, 0.0722);
float luma(vec3 c) { return dot(c, W); }
vec3 toLin(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c)); }
vec3 toSrgb(vec3 c) { c = max(c, 0.0); return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c)); }
float hash(vec2 p) { p = fract(p * vec2(443.897, 441.423)); p += dot(p, p.yx + 19.19); return fract((p.x + p.y) * p.x); }

// Output pixel -> source UV (or outside 0..1 when the point falls off the photo).
vec2 sourceUV(vec2 outPx) {
  vec2 r = uCrop.xy + (outPx / uOutSize) * uCrop.zw;          // rotated space, 0..1
  vec2 p = (r - 0.5) * uRotSize;                               // centered pixels
  float s = sin(-uAngle), c = cos(-uAngle);
  p = mat2(c, s, -s, c) * p;                                   // undo straighten
  vec2 q = p / uRotSize;                                       // keystone (perspective)
  q = vec2(q.x * (1.0 + uPers.y * q.y * 1.2), q.y * (1.0 + uPers.x * q.x * 1.2));
  p = q * uRotSize * uFlip;                                    // undo flips
  for (int i = 0; i < 4; i++) { if (i < uQuarter) p = vec2(p.y, -p.x); } // undo quarter turns
  return p / uSrcSize + 0.5;
}

float localMean(vec2 uv) {
  float l = uLocalLod;
  vec2 d = exp2(l) / uSrcSize;
  return 0.2 * (luma(textureLod(uSrc, uv, l).rgb) + luma(textureLod(uSrc, uv + vec2(d.x, 0.0), l).rgb) +
    luma(textureLod(uSrc, uv - vec2(d.x, 0.0), l).rgb) + luma(textureLod(uSrc, uv + vec2(0.0, d.y), l).rgb) +
    luma(textureLod(uSrc, uv - vec2(0.0, d.y), l).rgb));
}

void main() {
  vec2 frag = vec2(gl_FragCoord.x, uTarget.y - gl_FragCoord.y);
  vec2 outPx = (frag - uViewOffset) / uViewScale;
  if (any(lessThan(outPx, vec2(0.0))) || any(greaterThanEqual(outPx, uOutSize))) { outColor = vec4(0.0); return; }
  vec2 uv = sourceUV(outPx);
  if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) { outColor = vec4(0.0, 0.0, 0.0, 1.0); return; }

  float exposure = uA[0] * 0.02, brilliance = uA[1] * 0.01, highlights = uA[2] * 0.01, shadows = uA[3] * 0.01;
  float contrast = uA[4] * 0.01, brightness = uA[5] * 0.01, blackPoint = uA[6] * 0.01;
  float saturation = uA[7] * 0.01, vibrance = uA[8] * 0.01, warmth = uA[9] * 0.01, tint = uA[10] * 0.01;
  float sharp = uA[11] * 0.01, definition = uA[12] * 0.01, nr = uA[13] * 0.01, vignette = uA[14] * 0.01;
  float bw = uA[15] * 0.01, bwNeutrals = uA[16] * 0.01, bwTone = uA[17] * 0.01, grain = uA[18] * 0.01;

  vec4 src = texture(uSrc, uv);
  vec3 col = src.rgb;
  vec2 tx = 1.0 / uSrcSize;

  // Noise reduction: edge-preserving 3x3 average.
  if (nr > 0.0) {
    vec3 sum = vec3(0.0); float wsum = 0.0;
    for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
      vec3 n = texture(uSrc, uv + vec2(x, y) * tx).rgb;
      vec3 d = n - col;
      float w = exp(-dot(d, d) * (60.0 - 45.0 * nr));
      sum += n * w; wsum += w;
    }
    col = mix(col, sum / wsum, nr);
  }
  // Sharpness: unsharp mask on the 4 neighbours.
  if (sharp > 0.0) {
    vec3 blur = 0.25 * (texture(uSrc, uv + vec2(tx.x, 0.0)).rgb + texture(uSrc, uv - vec2(tx.x, 0.0)).rgb +
      texture(uSrc, uv + vec2(0.0, tx.y)).rgb + texture(uSrc, uv - vec2(0.0, tx.y)).rgb);
    col += (col - blur) * sharp * 1.6;
  }

  // Exposure and white balance act on light, so they work in linear.
  vec3 lin = toLin(clamp(col, 0.0, 1.0)) * exp2(exposure);
  vec3 wb = vec3(1.0 + 0.18 * warmth, 1.0 - 0.10 * tint, 1.0 - 0.18 * warmth);
  lin *= wb / luma(wb);
  vec3 p = toSrgb(lin);
  float L = toSrgb(vec3(toLin(vec3(localMean(uv))) * exp2(exposure))).r;

  // Black point
  float bp = blackPoint * 0.12;
  p = (p - bp) / (1.0 - bp);
  L = (L - bp) / (1.0 - bp);

  // Tone: shadows / highlights use the neighbourhood's brightness, so they recover detail
  // without flattening the photo. Brilliance mixes all three, as in Photos.
  float sh = shadows + brilliance * 0.45;
  float hi = highlights - brilliance * 0.35;
  float def = definition + max(brilliance, 0.0) * 0.25;
  float Y = clamp(luma(p), 0.0, 1.0);
  float Yn = Y;
  float mS = 1.0 - smoothstep(0.0, 0.6, L);
  float mH = smoothstep(0.4, 1.0, L);
  Yn = pow(Yn, exp2(-sh * 1.1 * mS));
  Yn = 1.0 - pow(1.0 - Yn, exp2(hi * 1.1 * mH));
  Yn = pow(Yn, exp2(-brightness * 0.7));
  float mid = 1.0 - abs(2.0 * Yn - 1.0);
  Yn += contrast * 0.9 * (Yn - 0.5) * mid;
  Yn += def * 1.4 * (Y - L) * mid;
  Yn = clamp(Yn, 0.0, 1.0);
  p *= (Yn + 0.02) / (Y + 0.02);

  // Color
  float Yp = luma(p);
  float s = max(p.r, max(p.g, p.b)) - min(p.r, min(p.g, p.b));
  p = mix(vec3(Yp), p, 1.0 + vibrance * (1.0 - clamp(s, 0.0, 1.0)) * 1.2);
  p = mix(vec3(Yp), p, 1.0 + saturation);

  // Black & white
  if (bw > 0.0) {
    float g = clamp(luma(p), 0.0, 1.0);
    g = pow(g, exp2(-bwNeutrals * 0.6));
    g += bwTone * 0.8 * (g - 0.5) * (1.0 - abs(2.0 * g - 1.0));
    p = mix(p, vec3(g), bw);
  }

  // Vignette (in the cropped frame)
  if (vignette != 0.0) {
    vec2 v = (outPx / uOutSize - 0.5) * vec2(uOutSize.x / max(uOutSize.x, uOutSize.y), uOutSize.y / max(uOutSize.x, uOutSize.y));
    float f = smoothstep(0.25, 0.75, length(v) * 1.15);
    p = vignette > 0.0 ? p * (1.0 - vignette * 0.8 * f) : mix(p, vec3(1.0), -vignette * 0.5 * f);
  }

  // Grain, anchored to the photo's own pixels so it looks the same in preview and export.
  if (grain > 0.0) {
    float n = hash(floor(outPx)) + hash(floor(outPx) + 17.0) - 1.0;
    p += n * grain * 0.12 * (1.0 - 0.6 * abs(2.0 * luma(p) - 1.0));
  }

  outColor = vec4(clamp(p, 0.0, 1.0), src.a);
}`;

export interface View {
  /** Target pixels per output pixel. */
  scale: number;
  /** Where the output's top-left lands in the target, target pixels. */
  x: number;
  y: number;
}

export interface Size {
  w: number;
  h: number;
}

/** Output size of the edited photo (after rotation and crop), in pixels. */
export function outputSize(src: Size, g: Geometry): Size {
  const rot = g.quarter % 2 ? { w: src.h, h: src.w } : src;
  return { w: Math.max(1, Math.round(rot.w * g.crop.w)), h: Math.max(1, Math.round(rot.h * g.crop.h)) };
}

export class Pipeline {
  readonly gl: WebGL2RenderingContext;
  private prog: WebGLProgram;
  private tex: WebGLTexture | null = null;
  private loc: Record<string, WebGLUniformLocation | null> = {};
  src: Size = { w: 0, h: 0 };
  readonly maxSize: number;

  constructor(readonly canvas: HTMLCanvasElement | OffscreenCanvas, colorSpace: PredefinedColorSpace = "srgb") {
    const gl = canvas.getContext("webgl2", { premultipliedAlpha: false, antialias: false, preserveDrawingBuffer: true }) as WebGL2RenderingContext | null;
    if (!gl) throw new Error("This computer’s graphics don’t support photo editing (WebGL 2 is unavailable).");
    this.gl = gl;
    if ("drawingBufferColorSpace" in gl) (gl as any).drawingBufferColorSpace = colorSpace;
    this.maxSize = gl.getParameter(gl.MAX_TEXTURE_SIZE);
    this.prog = this.link(VERT, FRAG);
    gl.useProgram(this.prog);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const aPos = gl.getAttribLocation(this.prog, "aPos");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
    for (const n of ["uSrc", "uSrcSize", "uLocalLod", "uTarget", "uViewOffset", "uViewScale", "uOutSize", "uCrop", "uRotSize", "uAngle", "uQuarter", "uFlip", "uPers", "uA"]) {
      this.loc[n] = gl.getUniformLocation(this.prog, n);
    }
  }

  private link(vs: string, fs: string): WebGLProgram {
    const gl = this.gl;
    const compile = (type: number, src: string) => {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader error");
      return s;
    };
    const p = gl.createProgram()!;
    gl.attachShader(p, compile(gl.VERTEX_SHADER, vs));
    gl.attachShader(p, compile(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? "link error");
    return p;
  }

  /** Uploads the photo (already oriented upright). */
  setSource(image: TexImageSource, size: Size) {
    const gl = this.gl;
    if (this.tex) gl.deleteTexture(this.tex);
    this.tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    gl.pixelStorei(gl.UNPACK_COLORSPACE_CONVERSION_WEBGL, gl.NONE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE, image);
    gl.generateMipmap(gl.TEXTURE_2D);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    this.src = size;
  }

  private setUniforms(edits: Edits, target: Size, view: View, neutral: boolean) {
    const gl = this.gl;
    const g = edits.geometry;
    const out = outputSize(this.src, g);
    const rot = g.quarter % 2 ? { w: this.src.h, h: this.src.w } : this.src;
    const a = neutral ? null : effectiveAdjust(edits);
    gl.uniform1i(this.loc.uSrc, 0);
    gl.uniform2f(this.loc.uSrcSize, this.src.w, this.src.h);
    gl.uniform1f(this.loc.uLocalLod, Math.max(0, Math.log2(Math.max(this.src.w, this.src.h) / 40)));
    gl.uniform2f(this.loc.uTarget, target.w, target.h);
    gl.uniform2f(this.loc.uViewOffset, view.x, view.y);
    gl.uniform1f(this.loc.uViewScale, view.scale);
    gl.uniform2f(this.loc.uOutSize, out.w, out.h);
    gl.uniform4f(this.loc.uCrop, g.crop.x, g.crop.y, g.crop.w, g.crop.h);
    gl.uniform2f(this.loc.uRotSize, rot.w, rot.h);
    gl.uniform1f(this.loc.uAngle, (g.angle * Math.PI) / 180);
    gl.uniform1i(this.loc.uQuarter, g.quarter % 4);
    gl.uniform2f(this.loc.uFlip, g.flipH ? -1 : 1, g.flipV ? -1 : 1);
    gl.uniform2f(this.loc.uPers, g.persH / 100, g.persV / 100);
    gl.uniform1fv(this.loc.uA, new Float32Array(ADJUST_ORDER.map((k) => (a ? a[k] : 0))));
  }

  /** Draws into the canvas. `neutral` shows the original (hold to compare), keeping geometry. */
  render(edits: Edits, view: View, neutral = false) {
    const gl = this.gl;
    if (!this.tex) return;
    const target = { w: gl.drawingBufferWidth, h: gl.drawingBufferHeight };
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, target.w, target.h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    this.setUniforms(edits, target, view, neutral);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  /**
   * Renders the edited photo at full resolution and returns its RGBA pixels (top row first).
   * Large photos render in tiles so no single draw exceeds the GPU's limits.
   */
  renderFull(edits: Edits, scale = 1): { data: Uint8Array; w: number; h: number } {
    const gl = this.gl;
    const full = outputSize(this.src, edits.geometry);
    const w = Math.max(1, Math.round(full.w * scale));
    const h = Math.max(1, Math.round(full.h * scale));
    const data = new Uint8Array(w * h * 4);
    const TILE = Math.min(4096, this.maxSize);
    const fbTex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, fbTex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, TILE, TILE);
    const fb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, fbTex, 0);
    const row = new Uint8Array(TILE * TILE * 4);
    try {
      for (let ty = 0; ty < h; ty += TILE) {
        for (let tx = 0; tx < w; tx += TILE) {
          const tw = Math.min(TILE, w - tx);
          const th = Math.min(TILE, h - ty);
          gl.viewport(0, 0, tw, th);
          gl.activeTexture(gl.TEXTURE0);
          gl.bindTexture(gl.TEXTURE_2D, this.tex);
          // The shader flips y against uTarget, so give it the tile height.
          this.setUniforms(edits, { w: tw, h: th }, { scale, x: -tx, y: -ty }, false);
          gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
          gl.readPixels(0, 0, tw, th, gl.RGBA, gl.UNSIGNED_BYTE, row);
          // readPixels returns bottom row first.
          for (let y = 0; y < th; y++) {
            const srcRow = (th - 1 - y) * tw * 4;
            data.set(row.subarray(srcRow, srcRow + tw * 4), ((ty + y) * w + tx) * 4);
          }
        }
      }
    } finally {
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.deleteFramebuffer(fb);
      gl.deleteTexture(fbTex);
    }
    return { data, w, h };
  }

  dispose() {
    const gl = this.gl;
    if (this.tex) gl.deleteTexture(this.tex);
    gl.deleteProgram(this.prog);
    gl.getExtension("WEBGL_lose_context")?.loseContext();
  }
}
