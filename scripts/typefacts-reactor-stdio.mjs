#!/usr/bin/env -S node --no-warnings
// The Type Facts stdio service, served by a producer wasm reactor. It lets the
// differential harness (scripts/typefacts-differential.mjs replay --candidate ...)
// and anything else that spawns a producer drive the wasip1 reactor with the
// recorded request streams: same flags, same startup handshake, one response
// frame per request frame.
//
//   TYPEFACTS_REACTOR_WASM=packages/wasm/owned/solid-typefacts.wasm \
//     typefacts-reactor-stdio.mjs -project /abs/tsconfig.json [-project-config FILE]
//
// Differential run against the Rust reactor (build it with
// `make build-wasm-package`; the wasm is packages/wasm/owned/solid-typefacts.wasm):
//   make build-typefacts build-typefacts-transcript build-checker-debug
//   bun scripts/typefacts-differential.mjs record --gate coverage --out rust/target/differential/transcripts
//   bun scripts/typefacts-differential.mjs run rust/target/differential/transcripts/coverage \
//     --candidate "$PWD/scripts/typefacts-reactor-stdio.mjs"
// The module is read from TYPEFACTS_REACTOR_WASM, an absolute path.
//
// Flags the reactor has no use for are accepted and ignored: -schema,
// -inline-sources (a reactor always answers sources inline), -transition-arena
// (it has no arena; the client takes the transition inline) and -cpuprofile.
// The reactor opens its file system through WASI preopens, here the host's own
// root, so a recorded project is read at the paths it was recorded at.
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { encodeCbor, loadReactor } from "./lib/typefacts-reactor.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const wasm = process.env.TYPEFACTS_REACTOR_WASM || join(root, "packages/wasm/owned/solid-typefacts.wasm");

function fail(message) {
  process.stderr.write(`solid-typefacts: ${message}\n`);
  process.exit(1);
}

const options = { project: "", config: "" };
const args = process.argv.slice(2);
for (let index = 0; index < args.length; index++) {
  const [flag, inline] = args[index].replace(/^--?/, "").split(/=(.*)/s);
  const value = () => inline ?? args[++index];
  if (flag === "project") options.project = value();
  else if (flag === "project-config") options.config = value();
  else if (["schema", "transition-arena", "cpuprofile"].includes(flag)) value();
  else if (flag !== "inline-sources") fail(`unknown flag ${args[index]}`);
}
if (!options.project) fail("-project is required");

const reactor = await loadReactor(wasm);
const writeFrame = frame => new Promise(done => process.stdout.write(frame, done));
await writeFrame(reactor.handshake());

let handle;
try {
  const envelope = { projectId: resolve(options.project) };
  if (options.config) {
    envelope.hasConfig = true;
    envelope.config = readFileSync(options.config);
  }
  handle = reactor.open(encodeCbor(envelope));
} catch (error) {
  fail(error.message);
}

let pending = Buffer.alloc(0);
process.stdin.on("data", chunk => {
  pending = Buffer.concat([pending, chunk]);
  while (pending.length >= 4 && pending.length >= 4 + pending.readUInt32LE(0)) {
    const size = 4 + pending.readUInt32LE(0);
    const request = pending.subarray(0, size);
    pending = pending.subarray(size);
    let response;
    try {
      response = reactor.lifecycle(handle, request);
    } catch (error) {
      fail(error.message);
    }
    process.stdout.write(response);
  }
});
process.stdin.on("end", () => {
  try {
    reactor.close(handle);
  } catch (error) {
    fail(error.message);
  }
});
