#!/usr/bin/env node
// Keeps the app version identical in package.json, src-tauri/Cargo.toml (+ Cargo.lock) and
// src-tauri/tauri.conf.json.
//
//   node scripts/version.mjs            prints the versions, exits 1 if they differ (used by CI)
//   node scripts/version.mjs 0.2.0      sets every file to 0.2.0
//   node scripts/version.mjs --tag v0.2.0   exits 1 unless the files match the tag (release CI)

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const CRATE = "valet-linux-plus-desktop";

const files = {
  "package.json": {
    read: (s) => JSON.parse(s).version,
    write: (s, v) => s.replace(/("version":\s*")[^"]+(")/, `$1${v}$2`),
  },
  "src-tauri/tauri.conf.json": {
    read: (s) => JSON.parse(s).version,
    write: (s, v) => s.replace(/("version":\s*")[^"]+(")/, `$1${v}$2`),
  },
  "src-tauri/Cargo.toml": {
    read: (s) => s.match(/^\[package\][^[]*?^version\s*=\s*"([^"]+)"/m)?.[1],
    write: (s, v) => s.replace(/(^\[package\][^[]*?^version\s*=\s*")[^"]+(")/m, `$1${v}$2`),
  },
  "src-tauri/Cargo.lock": {
    read: (s) => s.match(new RegExp(`name = "${CRATE}"\\nversion = "([^"]+)"`))?.[1],
    write: (s, v) => s.replace(new RegExp(`(name = "${CRATE}"\\nversion = ")[^"]+(")`), `$1${v}$2`),
  },
};

const load = (file) => readFileSync(join(root, file), "utf8");
const versions = () => Object.fromEntries(Object.entries(files).map(([f, h]) => [f, h.read(load(f))]));
const print = (v) => Object.entries(v).forEach(([f, ver]) => console.log(`${(ver ?? "?").padEnd(14)} ${f}`));

const [arg, value] = process.argv.slice(2);

if (arg && arg !== "--tag") {
  if (!SEMVER.test(arg)) {
    console.error(`invalid version "${arg}" (expected x.y.z or x.y.z-pre)`);
    process.exit(2);
  }
  for (const [file, handler] of Object.entries(files)) {
    writeFileSync(join(root, file), handler.write(load(file), arg));
  }
}

const current = versions();
print(current);
const unique = new Set(Object.values(current));
if (unique.size !== 1 || unique.has(undefined)) {
  console.error("versions differ: run `node scripts/version.mjs <x.y.z>`");
  process.exit(1);
}
if (arg === "--tag") {
  const tag = (value ?? "").replace(/^v/, "");
  if (tag !== [...unique][0]) {
    console.error(`tag v${tag} does not match the app version ${[...unique][0]}`);
    process.exit(1);
  }
}
