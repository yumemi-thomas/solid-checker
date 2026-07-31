import { spawnSync } from "node:child_process";
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
process.exit(result.status ?? 1);
