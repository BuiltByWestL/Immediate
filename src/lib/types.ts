// Shapes shared with the Rust core (src-tauri/src). Keep in sync.

export interface Settings {
  theme: string;
  systemLight: string;
  systemDark: string;
  accent: string | null;
  mode: "simple" | "advanced";
  font: "immediate" | "helvetica" | "system";
  reduceMotion: boolean;
  tintCanvas: boolean;
  checkUpdates: boolean;
  saveSuffix: string;
  removeLocation: boolean;
  infoOpen: boolean;
  filmstrip: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  systemLight: "paper",
  systemDark: "graphite",
  accent: null,
  mode: "simple",
  font: "immediate",
  reduceMotion: false,
  tintCanvas: false,
  checkUpdates: true,
  saveSuffix: " edited",
  removeLocation: false,
  infoOpen: false,
  filmstrip: true,
};

export type Kind = "image" | "video" | "audio" | "document";

export interface OpenedFile {
  path: string;
  name: string;
  dir: string;
  ext: string;
  kind: Kind;
  editable: boolean;
  size: number;
  modified: number | null;
  temporary: boolean;
}

export interface Details {
  created: number | null;
  exif: [string, string][];
  location: [number, number] | null;
}

export interface Batch {
  files: string[];
  append: boolean;
}

export interface Tool {
  path: string;
  version: string;
}
export interface ToolsStatus {
  ffmpeg: Tool | null;
  ffprobe: Tool | null;
}

/** What the viewer learned from the media itself (decoded size, duration). */
export interface MediaFacts {
  width?: number;
  height?: number;
  duration?: number;
  playable?: boolean;
}

declare global {
  interface Window {
    /** Injected before the page loads (src-tauri/src/themes.rs). Absent in a plain browser. */
    __IMMEDIATE__?: { settings: Settings; platform: string };
  }
}
