#!/usr/bin/env node
// Dispatch the sidecar preparation to the right script for this platform.
// We keep a `.sh` and a `.ps1` so each platform uses its native shell.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { platform } from "node:process";

const here = dirname(fileURLToPath(import.meta.url));
const sidecarDir = resolve(here, "..", "src-tauri");

const isWindows = platform === "win32";
const command = isWindows ? "powershell" : "bash";
const args = isWindows
  ? ["-ExecutionPolicy", "Bypass", "-File", "prepare-sidecar.ps1"]
  : ["prepare-sidecar.sh"];

const result = spawnSync(command, args, { cwd: sidecarDir, stdio: "inherit", shell: false });
process.exit(result.status ?? 1);