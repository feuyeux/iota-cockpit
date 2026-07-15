import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(here, "..");
const packagePath = resolve(desktopRoot, "package.json");
const tauriConfigPath = resolve(desktopRoot, "src-tauri", "tauri.conf.json");

function nextPatchVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`Expected a semantic version in major.minor.patch form, received: ${version}`);
  }
  const [, major, minor, patch] = match;
  return `${major}.${minor}.${Number(patch) + 1}`;
}

const [packageJson, tauriConfig] = await Promise.all(
  [packagePath, tauriConfigPath].map(async (path) => JSON.parse(await readFile(path, "utf8")))
);

if (packageJson.version !== tauriConfig.version) {
  throw new Error(
    `Desktop package version (${packageJson.version}) does not match Tauri version (${tauriConfig.version}).`
  );
}

const version = nextPatchVersion(packageJson.version);
packageJson.version = version;
tauriConfig.version = version;

await Promise.all([
  writeFile(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`),
  writeFile(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`),
]);

console.log(`Cockpit desktop bundle version: ${version}`);
