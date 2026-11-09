// Prints the notes for a version from CHANGELOG.md (its "## <version>" section), for the
// release page and the in-app update offer. Fails if there's no section, so every release has
// notes.
//
//   node scripts/release-notes.mjs v0.3.0

import { readFileSync } from "node:fs";

const version = (process.argv[2] ?? "").replace(/^v/, "");
if (!version) {
  console.error("Usage: node scripts/release-notes.mjs <version>");
  process.exit(2);
}
const text = readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8").replace(/\r\n/g, "\n");
const start = text.search(new RegExp(`^## ${version.replace(/\./g, "\\.")}\\s*$`, "m"));
if (start < 0) {
  console.error(`CHANGELOG.md has no "## ${version}" section. Add the notes for this release first.`);
  process.exit(1);
}
const body = text.slice(start).split("\n").slice(1);
const end = body.findIndex((l) => /^## /.test(l));
process.stdout.write((end < 0 ? body : body.slice(0, end)).join("\n").trim() + "\n");
