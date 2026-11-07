// Filmstrip thumbnails: fetched from Rust a few at a time, kept as blob URLs (most recent 300).

import { invoke } from "@tauri-apps/api/core";

const SIZE = 128;
const LIMIT = 300;
const cache = new Map<string, string | null>(); // null = no thumbnail available
const waiting = new Map<string, ((url: string | null) => void)[]>();
const queue: string[] = [];
let running = 0;

export function cachedThumb(path: string): string | null | undefined {
  return cache.get(path);
}

export function thumb(path: string): Promise<string | null> {
  const hit = cache.get(path);
  if (hit !== undefined) return Promise.resolve(hit);
  return new Promise((resolve) => {
    const list = waiting.get(path);
    if (list) list.push(resolve);
    else {
      waiting.set(path, [resolve]);
      queue.push(path);
      pump();
    }
  });
}

/** Drops queued requests that scrolled out of view. */
export function keepOnly(paths: Set<string>) {
  for (let i = queue.length - 1; i >= 0; i--) {
    if (!paths.has(queue[i])) {
      const p = queue[i];
      queue.splice(i, 1);
      waiting.get(p)?.forEach((r) => r(null));
      waiting.delete(p);
    }
  }
}

function pump() {
  while (running < 3 && queue.length) {
    const path = queue.shift()!;
    running++;
    invoke<ArrayBuffer>("thumbnail", { path, size: SIZE })
      .then((buf) => URL.createObjectURL(new Blob([buf], { type: "image/png" })))
      .catch(() => null)
      .then((url) => {
        cache.set(path, url);
        if (cache.size > LIMIT) {
          const [oldest, u] = cache.entries().next().value!;
          cache.delete(oldest);
          if (u) URL.revokeObjectURL(u);
        }
        waiting.get(path)?.forEach((r) => r(url));
        waiting.delete(path);
      })
      .finally(() => {
        running--;
        pump();
      });
  }
}
