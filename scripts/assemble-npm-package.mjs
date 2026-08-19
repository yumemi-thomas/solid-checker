#!/usr/bin/env node

import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { basename, join, resolve } from "node:path";

const [inputArg, outputArg, version] = process.argv.slice(2);
if (!inputArg || !outputArg || !version) {
  throw new Error("usage: assemble-npm-package.mjs INPUT_DIR OUTPUT_DIR VERSION");
}

const input = resolve(inputArg);
const output = resolve(outputArg);
const targets = [
  "linux-x64",
  "linux-arm64",
  "darwin-arm64",
  "win32-x64"
];
const packageSuffixes = {
  "linux-x64": "linux-x64-gnu",
  "linux-arm64": "linux-arm64-gnu",
  "darwin-arm64": "darwin-arm64",
  "win32-x64": "win32-x64-msvc"
};

rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });

const first = join(input, targets[0]);
const mainOutput = join(output, "solid-checker");
cpSync(first, mainOutput, {
  recursive: true,
  filter(source) {
    return basename(source) !== "native-manifest.json" && basename(source) !== "native";
  }
});

const packageJsonPath = join(mainOutput, "package.json");
const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
packageJson.version = version;
packageJson.dependencies = {
  "solid-checker-wasm": `^${version}`
};
packageJson.optionalDependencies = {};
for (const target of targets) {
  packageJson.optionalDependencies[
    `@solid-checker/binding-${packageSuffixes[target]}`
  ] = `^${version}`;
}
writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);

const manifests = [];
for (const target of targets) {
  const directory = join(input, target);
  if (!existsSync(directory)) throw new Error(`missing release artifact: ${target}`);
  const nativePackageName = `@solid-checker/binding-${packageSuffixes[target]}`;
  const nativeOutput = join(output, `binding-${packageSuffixes[target]}`);
  cpSync(join(directory, "native", target), join(nativeOutput, "native", target), {
    recursive: true
  });
  const manifest = JSON.parse(readFileSync(join(directory, "native-manifest.json"), "utf8"));
  manifests.push(manifest);
  const [platform, arch] = target.split("-");
  if (platform !== "win32") {
    for (const binary of Object.values(manifest.binaries)) {
      chmodSync(join(nativeOutput, binary.path), 0o755);
    }
  }
  writeFileSync(join(nativeOutput, "package.json"), `${JSON.stringify({
    name: nativePackageName,
    version,
    description: `Native executables for solid-checker on ${platform}-${arch}`,
    os: [platform],
    cpu: [arch],
    files: ["native", "native-manifest.json"],
    license: packageJson.license,
    repository: packageJson.repository,
    publishConfig: packageJson.publishConfig
  }, null, 2)}\n`);
  writeFileSync(
    join(nativeOutput, "native-manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`
  );
}

writeFileSync(
  join(mainOutput, "native-manifest.json"),
  `${JSON.stringify({ schema: 1, version, targets: manifests }, null, 2)}\n`
);
