// Build the updater manifest (latest.json) for a GitHub release and publish it to the
// `updates` branch (an orphan branch holding only that file).
//
// Usage: node scripts/gen-latest-json.mjs <tag> [--public] [--dry-run]
//   --public   use browser download URLs (once the repo is public; no token needed)
//   default    use API asset URLs, which work for a private repo with a token header
//
// Needs the GitHub CLI (`gh`), authenticated with access to the repo.

import { execFileSync } from "node:child_process";

const REPO = process.env.IMMEDIATE_REPO ?? "BuiltByWestL/Immediate";
const args = process.argv.slice(2);
const tag = args.find((a) => !a.startsWith("--"));
const isPublic = args.includes("--public");
const dryRun = args.includes("--dry-run");
if (!tag) {
  console.error("usage: node scripts/gen-latest-json.mjs <tag> [--public] [--dry-run]");
  process.exit(2);
}

const gh = (ghArgs, input) =>
  execFileSync("gh", ghArgs, { encoding: "utf8", input, stdio: ["pipe", "pipe", "inherit"] });
const api = (path, { method = "GET", body, raw = false } = {}) => {
  const a = ["api", "-X", method, path];
  if (raw) a.push("-H", "Accept: application/octet-stream");
  if (body) a.push("--input", "-");
  const out = gh(a, body ? JSON.stringify(body) : undefined);
  return raw ? out : JSON.parse(out || "null");
};

// Updater artifact name patterns (Tauri bundler output) -> updater platform keys.
const PLATFORMS = [
  [/_x64-setup\.exe$/, "windows-x86_64"],
  [/_arm64-setup\.exe$/, "windows-aarch64"],
  [/_aarch64\.app\.tar\.gz$/, "darwin-aarch64"],
  [/_x64\.app\.tar\.gz$/, "darwin-x86_64"],
  [/_amd64\.AppImage$/, "linux-x86_64"],
  [/_aarch64\.AppImage$/, "linux-aarch64"],
];

const release = api(`repos/${REPO}/releases/tags/${tag}`);
const assets = release.assets;
const platforms = {};
for (const asset of assets) {
  const match = PLATFORMS.find(([re]) => re.test(asset.name));
  if (!match) continue;
  const sig = assets.find((a) => a.name === `${asset.name}.sig`);
  if (!sig) {
    console.warn(`skip ${asset.name}: no .sig (was it built with the signing key?)`);
    continue;
  }
  platforms[match[1]] = {
    signature: api(`repos/${REPO}/releases/assets/${sig.id}`, { raw: true }).trim(),
    url: isPublic ? asset.browser_download_url : asset.url,
  };
  console.log(`${match[1].padEnd(16)} ${asset.name}`);
}
if (!Object.keys(platforms).length) {
  console.error(`No signed updater artifacts found in release ${tag}.`);
  process.exit(1);
}

const manifest = {
  version: tag.replace(/^v/, ""),
  notes: release.body || "",
  pub_date: release.published_at ?? new Date().toISOString(),
  platforms,
};
const json = JSON.stringify(manifest, null, 2) + "\n";
if (dryRun) {
  console.log(json);
  process.exit(0);
}

// Commit latest.json as the only file on the `updates` branch (Git Data API, no checkout).
let parent = null;
try {
  parent = api(`repos/${REPO}/git/ref/heads/updates`).object.sha;
} catch {
  /* branch does not exist yet */
}
const blob = api(`repos/${REPO}/git/blobs`, { method: "POST", body: { content: json, encoding: "utf-8" } });
const tree = api(`repos/${REPO}/git/trees`, {
  method: "POST",
  body: { tree: [{ path: "latest.json", mode: "100644", type: "blob", sha: blob.sha }] },
});
const commit = api(`repos/${REPO}/git/commits`, {
  method: "POST",
  body: { message: `Update manifest for ${tag}`, tree: tree.sha, parents: parent ? [parent] : [] },
});
if (parent) {
  api(`repos/${REPO}/git/refs/heads/updates`, { method: "PATCH", body: { sha: commit.sha } });
} else {
  api(`repos/${REPO}/git/refs`, { method: "POST", body: { ref: "refs/heads/updates", sha: commit.sha } });
}
console.log(`Published latest.json for ${tag} to the updates branch (${commit.sha.slice(0, 7)}).`);
