#!/usr/bin/env bun

import {
  cpSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { createHash } from "node:crypto";
import { basename, join, resolve } from "node:path";
import process from "node:process";

const root = resolve(import.meta.dirname, "..");
const options = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  const value = process.argv[index + 1];
  if (!key?.startsWith("--") || value === undefined) {
    throw new Error("usage: package-rust.mjs --output DIR [--platform NAME --arch NAME]");
  }
  options.set(key.slice(2), value);
}

const output = resolve(options.get("output") || join(root, "dist", "solid-checker"));
const version = options.get("version");
const platform = options.get("platform") || process.platform;
const arch = options.get("arch") || process.arch;
const extension = platform === "win32" ? ".exe" : "";
const rustDirectory = resolve(options.get("rust-directory") || join(root, "rust", "target", "release"));
const typefacts = resolve(options.get("typefacts") || join(root, "bin", `solid-typefacts${extension}`));
const nativeDirectory = join(output, "native", `${platform}-${arch}`);

rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });
cpSync(join(root, "packages", "cli"), output, {
  recursive: true,
  filter(source) {
    const name = basename(source);
    return !["node_modules", "native", "test", "package-lock.json", "bun.lock"].includes(name);
  }
});
if (version) {
  const packageJsonPath = join(output, "package.json");
  const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  packageJson.version = version;
  writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);
}
mkdirSync(nativeDirectory, { recursive: true });

const binaries = [
  ["solid-checker", join(rustDirectory, `solid-checker-rust${extension}`)],
  ["solid-typefacts", typefacts]
];
const manifest = {
  schema: 1,
  buildId: process.env.SOLID_CHECKER_BUILD_ID || "dev",
  platform,
  arch,
  binaries: {}
};
for (const [name, source] of binaries) {
  if (!statSync(source).isFile()) throw new Error(`${source} is not a file`);
  const destination = join(nativeDirectory, `${name}${extension}`);
  cpSync(source, destination);
  const bytes = readFileSync(destination);
  manifest.binaries[name] = {
    path: `native/${platform}-${arch}/${name}${extension}`,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    bytes: bytes.length
  };
}
writeFileSync(join(output, "native-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
