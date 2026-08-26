import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import process from "node:process";

const args = [
  "build",
  "--manifest-path", "../../rust/crates/solid-checker-wasm/Cargo.toml",
  "--package-json-path", "package.json",
  "--output-dir", ".",
  "--package", "solid-checker-wasm",
  "--dts", "generated.d.ts",
  "--target", "wasm32-wasip1-threads",
  "--platform",
  "--esm"
];
if (process.argv.includes("--release")) args.push("--release");

const result = spawnSync("napi", args, {
  cwd: new URL("..", import.meta.url),
  env: {
    ...process.env,
    RUSTUP_TOOLCHAIN: "1.97"
  },
  stdio: "inherit"
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

function replaceGenerated(source, before, after, file) {
  if (source.includes(after)) return source;
  if (!source.includes(before)) {
    throw new Error(`NAPI-RS generated ${file} in an unexpected format; refusing to apply the Bun WASI compatibility patch`);
  }
  return source.replace(before, after);
}

// NAPI-RS emits a Node WASI loader, but Bun intentionally does not implement
// Node's reactor-only `wasi.initialize()` API. Keep the published Node path
// native and select @napi-rs/wasm-runtime's portable WASI implementation only
// when the generated loader is evaluated by Bun. Reapply this small patch
// after every build because the loader and worker are generated files.
const wasiLoader = new URL("../solid-checker-wasm.wasi.cjs", import.meta.url);
let loader = readFileSync(wasiLoader, "utf8");
loader = replaceGenerated(
  loader,
  "const { WASI: __nodeWASI } = require('node:wasi')",
  "const { WASI: __nodeWASI } = process.versions.bun\n  ? require('@napi-rs/wasm-runtime')\n  : require('node:wasi')",
  "solid-checker-wasm.wasi.cjs",
);
loader = replaceGenerated(
  loader,
  "  preopens: {\n    [__rootDir]: __rootDir,\n  }\n})",
  "  preopens: {\n    [__rootDir]: __rootDir,\n  },\n  ...(process.versions.bun ? { fs: __nodeFs } : {})\n})",
  "solid-checker-wasm.wasi.cjs",
);
writeFileSync(wasiLoader, loader);

const wasiWorker = new URL("../wasi-worker.mjs", import.meta.url);
let worker = readFileSync(wasiWorker, "utf8");
worker = replaceGenerated(
  worker,
  'import { WASI } from "node:wasi";',
  'import { WASI as NodeWASI } from "node:wasi";\nimport { WASI as CompatWASI } from "@napi-rs/wasm-runtime";',
  "wasi-worker.mjs",
);
worker = replaceGenerated(
  worker,
  'const { instantiateNapiModuleSync, MessageHandler, getDefaultContext } = require("@napi-rs/wasm-runtime");',
  'const { instantiateNapiModuleSync, MessageHandler, getDefaultContext } = require("@napi-rs/wasm-runtime");\nconst WASI = process.versions.bun ? CompatWASI : NodeWASI;',
  "wasi-worker.mjs",
);
worker = replaceGenerated(
  worker,
  "    const wasi = new WASI({\n      version: 'preview1',\n      env: process.env,\n      preopens: {\n        [__rootDir]: __rootDir,\n      },\n    });",
  "    const wasiOptions = {\n      version: 'preview1',\n      env: process.env,\n      preopens: {\n        [__rootDir]: __rootDir,\n      },\n    };\n    if (process.versions.bun) wasiOptions.fs = fs;\n    const wasi = new WASI(wasiOptions);",
  "wasi-worker.mjs",
);
writeFileSync(wasiWorker, worker);
