// Themes at runtime. The first paint is themed by the script Rust injects (src-tauri/src/themes.rs);
// this module re-applies on settings changes, OS light/dark changes, and drives the gallery.

import accentsJson from "./accents.json";
import type { Settings } from "../lib/types";

export const TOKEN_NAMES = [
  "bg", "panel", "raised", "hover", "line", "lineStrong", "text", "textDim",
  "accent", "onAccent", "focus", "danger", "canvas",
] as const;
export type Tokens = Record<(typeof TOKEN_NAMES)[number], string>;

export interface Theme {
  id: string;
  name: string;
  group: string;
  scheme: "light" | "dark";
  tokens: Tokens;
}

type ThemeFile = Omit<Theme, "id">;
const files = import.meta.glob<ThemeFile>("./themes/*.json", { eager: true, import: "default" });

/** Gallery order. */
const ORDER = ["paper", "graphite", "ink", "mist", "sand", "dusk", "sunset", "aurora", "midnight", "candy", "citrus"];
export const BUILTIN: Theme[] = ORDER.map((id) => ({ id, ...files[`./themes/${id}.json`] }));
export const GROUPS = ["Monochrome", "Faint", "Colorful"];

type AccentPair = { accent: string; onAccent: string };
export interface Accent {
  id: string;
  name: string;
  light: AccentPair;
  dark: AccentPair;
}
export const ACCENTS: Accent[] = Object.entries(accentsJson)
  .filter(([k]) => !k.startsWith("$"))
  .map(([id, a]) => ({ id, ...(a as Omit<Accent, "id">) }));

let custom: Theme[] = [];
export function setCustomThemes(list: Theme[]) {
  custom = list;
}
export function customThemes() {
  return custom;
}

export function findTheme(id: string): Theme | undefined {
  return BUILTIN.find((t) => t.id === id) ?? custom.find((t) => t.id === id);
}

export function withAccent(theme: Theme, accentId: string | null): Theme {
  const a = accentId ? ACCENTS.find((x) => x.id === accentId) : undefined;
  if (!a) return theme;
  const v = a[theme.scheme];
  return { ...theme, tokens: { ...theme.tokens, accent: v.accent, focus: v.accent, onAccent: v.onAccent } };
}

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

export function resolveTheme(s: Settings, osDark = darkQuery.matches): Theme {
  const id = s.theme === "system" ? (osDark ? s.systemDark : s.systemLight) : s.theme;
  const theme = findTheme(id) ?? findTheme(osDark ? "graphite" : "paper")!;
  return withAccent(theme, s.accent);
}

const kebab = (k: string) => "--" + k.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());

/** Inline style that scopes a theme to one element (gallery previews). */
export function themeStyle(tokens: Tokens): string {
  return Object.entries(tokens)
    .map(([k, v]) => `${kebab(k)}:${v}`)
    .join(";");
}

let current: Settings | null = null;

export function applyTheme(s: Settings) {
  current = s;
  const el = document.documentElement;
  const t = resolveTheme(s);
  el.dataset.theme = t.id;
  el.dataset.scheme = t.scheme;
  el.dataset.font = s.font;
  el.toggleAttribute("data-reduce-motion", s.reduceMotion);
  el.toggleAttribute("data-tint-canvas", s.tintCanvas);
  for (const [k, v] of Object.entries(t.tokens)) el.style.setProperty(kebab(k), v);
}

darkQuery.addEventListener("change", () => {
  if (current?.theme === "system") applyTheme(current);
});
