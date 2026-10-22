// Builds the owned check's two reactors into owned/ and writes their manifest
// (P5 S2): the checker as `solid-checker-wasm` for wasm32-wasip1 with the
// `reactor` feature, and the Type Facts producer as a GOOS=wasip1 c-shared
// reactor, linked with the same build id so the startup handshake agrees.
//
// SOLID_TYPEFACTS_REACTOR selects the producer: `go` (the default) or `rust`
// (apps/solid-typefacts-rs built as a wasm32-wasip1 cdylib exporting the same
// ABI, migration slice S6). The checker, the host and the transport do not
// change with it; the manifest records which producer was built.
//
// The two builds run one after the other. Certification and probe pins are
// whatever the calling environment sets -- `make build-wasm-owned` supplies
// the same ones the native checker is built with -- and the manifest records
// which were set. Bridged producer sessions refuse certification, so no pin
// gives this build certification authority.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "../..");
const buildId = process.env.SOLID_CHECKER_BUILD_ID || "dev";
const reactor = process.env.SOLID_TYPEFACTS_REACTOR || "go";
if (reactor !== "go" && reactor !== "rust") {
  throw new Error(`build-owned: SOLID_TYPEFACTS_REACTOR is ${JSON.stringify(reactor)}, expected "go" or "rust"`);
}
const staging = join(repositoryRoot, "rust/target/wasm-owned");
const owned = join(packageRoot, "owned");
mkdirSync(staging, { recursive: true });
mkdirSync(owned, { recursive: true });

function run(command, args, options) {
  const result = spawnSync(command, args, { stdio: ["ignore", "inherit", "inherit"], ...options });
  if (result.error) throw new Error(`build-owned: ${command} could not run: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`build-owned: ${command} ${args.join(" ")} exited ${result.status}`);
}
function read(command, args, options) {
  const result = spawnSync(command, args, { encoding: "utf8", ...options });
  if (result.error || result.status !== 0) {
    throw new Error(`build-owned: ${command} ${args.join(" ")} failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout.trim();
}

const rustEnvironment = { ...process.env, RUSTUP_TOOLCHAIN: "1.99.1", SOLID_CHECKER_BUILD_ID: buildId, TYPEFACTS_BUILD_ID: buildId };
run("cargo", [
  "build", "--manifest-path", join(repositoryRoot, "rust/Cargo.toml"),
  "-p", "solid-checker-wasm", "--target", "wasm32-wasip1",
  "--no-default-features", "--features", "reactor,dialect-v2", "--release",
], { cwd: repositoryRoot, env: rustEnvironment });
const checkerBuilt = join(repositoryRoot, "rust/target/wasm32-wasip1/release/solid_checker_wasm.wasm");

// Both producers stage under their own names, so a build of one leaves the
// other's last module in place beside it.
const producerBuilt = join(staging, `solid-typefacts-${reactor}.wasm`);
const goEnvironment = { ...process.env, GOOS: "wasip1", GOARCH: "wasm", GOTOOLCHAIN: process.env.GOTOOLCHAIN || "local" };
const producerManifest = join(repositoryRoot, "apps/solid-typefacts-rs/Cargo.toml");
if (reactor === "go") {
  run("go", [
    "build", "-trimpath", "-buildmode=c-shared",
    `-ldflags=-s -w -X main.buildID=${buildId}`,
    "-o", `${producerBuilt}.partial`, "./apps/solid-typefacts",
  ], { cwd: repositoryRoot, env: goEnvironment });
  renameSync(`${producerBuilt}.partial`, producerBuilt);
} else {
  // The crate's library is an rlib for the native binaries and tests; the
  // module is that library alone compiled as a cdylib, so release LTO applies
  // to it. The 80 MiB shadow stack is tsrs's own wasm setting (its
  // notes/wasm-build.md).
  run("cargo", [
    "rustc", "--manifest-path", producerManifest, "--lib", "--crate-type", "cdylib",
    "--target", "wasm32-wasip1", "--release", "--features", "reactor", "--locked",
    // --wrap: see wasi_import_stubs in the producer's unsafe_boundary.rs.
    "--", "-C", "link-arg=-zstack-size=83886080", "-C", "link-arg=--wrap=__imported_wasi_snapshot_preview1_fd_filestat_set_times",
  ], { cwd: repositoryRoot, env: rustEnvironment });
  copyFileSync(join(repositoryRoot, "apps/solid-typefacts-rs/target/wasm32-wasip1/release/solid_typefacts.wasm"), producerBuilt);
}

const artifact = (source, file, details) => {
  const bytes = readFileSync(source);
  copyFileSync(source, join(owned, `${file}.partial`));
  renameSync(join(owned, `${file}.partial`), join(owned, file));
  return { file, sha256: createHash("sha256").update(bytes).digest("hex"), bytes: bytes.length, ...details };
};
const manifest = {
  format: 1,
  buildId,
  producerSource: JSON.parse(read("node", [
    join(repositoryRoot, "scripts/typefacts-source-identity.mjs"), "--build-id", buildId, ...(reactor === "rust" ? ["--rust"] : []),
  ], { cwd: repositoryRoot, env: reactor === "rust" ? rustEnvironment : process.env })),
  artifacts: {
    checker: artifact(checkerBuilt, "solid-checker.wasm", {
      crate: "solid-checker-wasm", target: "wasm32-wasip1", features: ["reactor", "dialect-v2"], profile: "release",
    }),
    producer: artifact(producerBuilt, "solid-typefacts.wasm", reactor === "rust"
      ? { package: "./apps/solid-typefacts-rs", crate: "solid-typefacts", target: "wasm32-wasip1", features: ["reactor"], profile: "release" }
      : { package: "./apps/solid-typefacts", goos: "wasip1", goarch: "wasm" }),
  },
  toolchains: {
    rustc: read("rustc", ["-vV"], { env: rustEnvironment }),
    ...(reactor === "go" ? { go: read("go", ["version"], { env: goEnvironment }) } : {}),
  },
};
writeFileSync(join(owned, "manifest.json.partial"), `${JSON.stringify(manifest, null, 2)}\n`);
renameSync(join(owned, "manifest.json.partial"), join(owned, "manifest.json"));
console.log(`build-owned: ${manifest.artifacts.checker.sha256} solid-checker.wasm, ${manifest.artifacts.producer.sha256} solid-typefacts.wasm (build id ${buildId})`);
