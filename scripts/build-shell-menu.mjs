// Builds the Windows 11 top-level right-click entry: the COM DLL (src-tauri/shell-ext) and the
// signed sparse package that registers it. Output in src-tauri/shell-ext/out/, bundled next to
// immediate.exe by tauri.windows.conf.json. Runs before every Windows build; does nothing elsewhere.
//
// Signing certificate (self-signed, subject CN=BuiltByWestL; the public half is
// src-tauri/shell-ext/package/immediate-menu.cer):
//   CI:    IMMEDIATE_MENU_PFX (base64 of the .pfx) and IMMEDIATE_MENU_PFX_PASSWORD
//   local: %USERPROFILE%\.tauri\immediate-menu.pfx and immediate-menu.pfx.password
// Without one the package is left unsigned and the Settings switch reports that it can't be added.
// Run it once before `cargo build`/`cargo test` on Windows: tauri-build checks that bundled files exist.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

if (process.platform !== "win32") process.exit(0);

const root = resolve(import.meta.dirname, "..");
const ext = join(root, "src-tauri", "shell-ext");
const out = join(ext, "out");
const stage = join(out, "package");
const run = (exe, args, opts = {}) => execFileSync(exe, args, { stdio: ["ignore", "inherit", "inherit"], ...opts });

run("cargo", ["build", "--release", "--lib", "--manifest-path", join(ext, "Cargo.toml")]);

rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });
copyFileSync(join(ext, "package", "AppxManifest.xml"), join(stage, "AppxManifest.xml"));
copyFileSync(join(root, "src-tauri", "icons", "Square150x150Logo.png"), join(stage, "logo.png"));
copyFileSync(join(root, "src-tauri", "icons", "Square44x44Logo.png"), join(stage, "logo44.png"));
copyFileSync(join(ext, "target", "release", "immediate_menu.dll"), join(out, "immediate_menu.dll"));

// The Windows SDK tools (present on GitHub's Windows runners and with Visual Studio).
function sdkTool(name) {
  const base = join(process.env["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)", "Windows Kits", "10", "bin");
  const versions = existsSync(base) ? readdirSync(base).filter((v) => /^10\./.test(v)).sort((a, b) => b.localeCompare(a, undefined, { numeric: true })) : [];
  for (const v of versions) {
    const p = join(base, v, "x64", name);
    if (existsSync(p)) return p;
  }
  throw new Error(`${name} not found; install the Windows 10/11 SDK`);
}

const msix = join(out, "immediate-menu.msix");
run(sdkTool("makeappx.exe"), ["pack", "/o", "/nv", "/d", stage, "/p", msix], { stdio: ["ignore", "ignore", "inherit"] });

let pfx = null;
let password = null;
if (process.env.IMMEDIATE_MENU_PFX) {
  pfx = join(tmpdir(), `immediate-menu-${process.pid}.pfx`);
  writeFileSync(pfx, Buffer.from(process.env.IMMEDIATE_MENU_PFX, "base64"));
  password = process.env.IMMEDIATE_MENU_PFX_PASSWORD ?? "";
} else {
  const local = join(homedir(), ".tauri", "immediate-menu.pfx");
  if (existsSync(local)) {
    pfx = local;
    password = readFileSync(`${local}.password`, "utf8").trim();
  }
}
if (pfx) {
  try {
    run(sdkTool("signtool.exe"), ["sign", "/q", "/fd", "SHA256", "/f", pfx, "/p", password, msix]);
  } finally {
    if (process.env.IMMEDIATE_MENU_PFX) rmSync(pfx, { force: true });
  }
  console.log("Built and signed the right-click package.");
} else {
  console.warn("warning: no signing certificate; the right-click package is unsigned and can't be registered.");
}
