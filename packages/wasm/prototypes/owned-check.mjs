// PROTOTYPE EVIDENCE (S1 of the WASM host protocol): run the shared ordinary
// check with package contracts disabled natively, and as the Rust checker
// reactor driving the canonical Type Facts producer reactor, over the same
// frozen project bytes at the same absolute paths, and compare the snapshots.
// Manual evidence only: not a maintained provider, not a gate, not
// certification authority. See owned-check.md beside this file.
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { WASI } from "@napi-rs/wasm-runtime";
import { memfs } from "@napi-rs/wasm-runtime/fs";

// @tybys/wasm-util logs expected WASI errno probes in development mode.
process.env.NODE_ENV ||= "production";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "../..");
const { values: options } = parseArgs({
  options: {
    "checker-wasm": { type: "string" },
    "native-checker": { type: "string" },
    typefacts: { type: "string" },
    scratch: { type: "string" },
    "update-frames": { type: "string" },
    only: { type: "string" },
  },
});
const checkerWasmPath = resolve(options["checker-wasm"] ??
  join(repositoryRoot, "rust/target/wasm32-wasip1/release/solid_checker_wasm.wasm"));
const nativeChecker = resolve(options["native-checker"] ?? join(repositoryRoot, "rust/target/debug/solid-checker-rust"));
const nativeTypefacts = resolve(options.typefacts ?? join(repositoryRoot, "bin/solid-typefacts"));
const only = options.only ? new Set(options.only.split(",")) : undefined;
const scratchRequest = options.scratch || "/tmp/sc-scratch.noindex/p5-owned-producer-roundtrip/owned-check";
mkdirSync(scratchRequest, { recursive: true });
// Realpath first: /tmp is a symlink on macOS, and the producer reports
// realpaths, so both sides must see the project at its real path.
const scratch = realpathSync(scratchRequest);
if (!relative(repositoryRoot, scratch).startsWith("..")) {
  // The whole apps/solid-typefacts tree is Type Facts source identity.
  throw new Error(`scratch ${scratch} must lie outside the repository`);
}
const out = join(scratch, `witness-${new Date().toISOString().replace(/[:.]/g, "-")}`);
mkdirSync(join(out, "bin"), { recursive: true });

const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const failures = [];
const records = [];
function check(condition, message) {
  if (!condition) failures.push(message);
  return condition;
}

// --- images ----------------------------------------------------------------
// The producer reactor is built here from the real producer package, with the
// build id the native producer's stamp records. The checker reactor is built
// beforehand with the certification pins (see owned-check.md); this harness
// never runs Cargo.
const buildinfo = JSON.parse(readFileSync(`${nativeTypefacts}.buildinfo`, "utf8"));
const goEnvironment = { ...process.env, GOPROXY: "off", GOTOOLCHAIN: "local" };
const producerWasmPath = join(out, "bin", "solid-typefacts.wasm");
{
  const result = spawnSync("go", [
    "build", "-trimpath", "-buildmode=c-shared",
    `-ldflags=-s -w -X main.buildID=${buildinfo.buildId}`,
    "-o", producerWasmPath, "./apps/solid-typefacts",
  ], { cwd: repositoryRoot, env: { ...goEnvironment, GOOS: "wasip1", GOARCH: "wasm" }, encoding: "utf8" });
  if (result.status !== 0) throw new Error(`producer reactor build failed:\n${result.stderr}`);
}
const goVersion = spawnSync("go", ["version"], { env: goEnvironment, encoding: "utf8" }).stdout.trim();
const sourceIdentity = JSON.parse(spawnSync("node", [
  join(repositoryRoot, "scripts/typefacts-source-identity.mjs"), "--build-id", buildinfo.buildId,
], { cwd: repositoryRoot, encoding: "utf8" }).stdout);
check(sourceIdentity.sourceDigest === buildinfo.sourceDigest,
  `native producer stamp ${buildinfo.sourceDigest} is stale against source ${sourceIdentity.sourceDigest}`);
const producerModule = await WebAssembly.compile(readFileSync(producerWasmPath));
const checkerModule = await WebAssembly.compile(readFileSync(checkerWasmPath));
// The Node that runs `tsc` and the source-identity script below: the first
// `node` on PATH, recorded by its real path and bytes.
const nodePath = realpathSync(spawnSync("node", ["-e", "process.stdout.write(process.execPath)"], { encoding: "utf8" }).stdout);
const images = {
  goVersion,
  sourceIdentity,
  node: { path: nodePath, sha256: sha256(readFileSync(nodePath)), bun: process.versions.bun },
  nativeProducer: { path: nativeTypefacts, sha256: sha256(readFileSync(nativeTypefacts)), buildinfo },
  producerReactor: { path: producerWasmPath, sha256: sha256(readFileSync(producerWasmPath)) },
  nativeChecker: { path: nativeChecker, sha256: sha256(readFileSync(nativeChecker)) },
  checkerReactor: { path: checkerWasmPath, sha256: sha256(readFileSync(checkerWasmPath)) },
};

// --- frozen project trees ----------------------------------------------------
function snapshot(root) {
  const entries = new Map();
  const walk = directory => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name);
      const stat = lstatSync(path);
      if (stat.isSymbolicLink()) entries.set(path, { link: readlinkSync(path) });
      else if (stat.isDirectory()) {
        entries.set(path, { directory: true });
        walk(path);
      } else entries.set(path, { bytes: readFileSync(path) });
    }
  };
  walk(root);
  return entries;
}
function manifest(entries) {
  return Object.fromEntries([...entries].map(([path, entry]) => [
    path,
    entry.bytes ? sha256(entry.bytes) : entry.link ? `link:${entry.link}` : "dir",
  ]));
}
// freeze copies a project into this run's own tree, lets the case mutate it
// before anything reads it, and records every byte: the bytes read here are
// the bytes mounted, and the disk tree is re-hashed after the native run.
function freeze(name, source, mutate = () => {}) {
  const caseRoot = join(out, "runs", name);
  const root = join(caseRoot, "project");
  mkdirSync(caseRoot, { recursive: true });
  if (typeof source === "string") cpSync(source, root, { recursive: true });
  else source(root);
  mutate(root);
  const entries = snapshot(root);
  const tree = { name, caseRoot, root, entries, manifest: manifest(entries) };
  writeFileSync(join(caseRoot, "mount-manifest.json"), `${JSON.stringify(tree.manifest, null, 2)}\n`);
  return tree;
}

// One case's request, from the project's own runtime metadata, in the native
// command line's shape and the reactor request's shape.
function caseRequest(tree, tsconfig = "tsconfig.json") {
  const projectId = join(tree.root, tsconfig);
  const metadataPath = join(tree.root, ".solid-checker", "runtime.json");
  const metadata = existsSync(metadataPath) ? JSON.parse(readFileSync(metadataPath, "utf8")) : {};
  const argv = ["--format", "json", "--project", projectId, "--no-package-contracts"];
  const runtime = {};
  for (const [key, flag] of [["target", "--runtime-target"], ["build", "--runtime-build"],
    ["rendering", "--rendering"], ["programBoundary", "--program-boundary"]]) {
    if (metadata[key] !== undefined) {
      argv.push(flag, metadata[key]);
      runtime[key] = metadata[key];
    }
  }
  for (const condition of metadata.conditions ?? []) argv.push("--runtime-condition", condition);
  for (const transform of metadata.frameworkTransforms ?? []) argv.push("--framework-transform", transform);
  if (metadata.conditions) runtime.conditions = metadata.conditions;
  if (metadata.frameworkTransforms) runtime.frameworkTransforms = metadata.frameworkTransforms;
  const request = { projectId, runtime, dependencySources: metadata.dependencySources ?? [] };
  for (const specifier of request.dependencySources) argv.push("--dependency-source", specifier);
  if (metadata.dependencySource === false) {
    argv.push("--no-dependency-source");
    request.noDependencySource = true;
  }
  if (metadata.realm !== undefined) {
    argv.push("--runtime-realm", metadata.realm);
    request.windowRealm = metadata.realm === "window";
  }
  return { projectId, argv, request };
}

function runNative(tree, request) {
  const result = spawnSync(nativeChecker, request.argv, {
    cwd: repositoryRoot,
    env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: nativeTypefacts },
    maxBuffer: 1 << 30,
  });
  writeFileSync(join(tree.caseRoot, "native.stdout"), result.stdout);
  writeFileSync(join(tree.caseRoot, "native.stderr"), result.stderr);
  const after = manifest(snapshot(tree.root));
  check(JSON.stringify(after) === JSON.stringify(tree.manifest), `${tree.name}: disk tree moved during the native run`);
  const stderr = result.stderr.toString("utf8");
  return {
    status: result.status,
    snapshot: result.status === 0 ? JSON.parse(result.stdout.toString("utf8")) : undefined,
    notes: stderr === "" ? [] : stderr.replace(/\n$/, "").split("\n"),
    stdoutSha256: sha256(result.stdout),
  };
}

// --- the owned host ------------------------------------------------------------
// One mounted namespace per check, shared by both reactors: the producer reads
// sources, configs and declarations from it, the checker reads manifests,
// runtime files and dependency sources from it, at the same absolute paths.
function mount(tree, omit = new Set()) {
  const { fs, vol } = memfs({}, "/");
  vol.mkdirSync(tree.root, { recursive: true });
  for (const [path, entry] of tree.entries) {
    if (omit.has(path)) continue;
    if (entry.directory) vol.mkdirSync(path, { recursive: true });
    else if (entry.link) vol.symlinkSync(entry.link, path);
    else vol.writeFileSync(path, entry.bytes);
  }
  return fs;
}

function frame(payload) {
  const bytes = Buffer.alloc(4 + payload.length);
  bytes.writeUInt32LE(payload.length, 0);
  payload.copy(bytes, 4);
  return bytes;
}

// cborEnd returns the offset just past the one data item at `offset`.
function cborEnd(bytes, offset) {
  const initial = bytes[offset];
  const major = initial >> 5;
  const info = initial & 0x1f;
  let at = offset + 1;
  let argument = info;
  if (info === 24) argument = bytes[at++];
  else if (info === 25) { argument = bytes.readUInt16BE(at); at += 2; }
  else if (info === 26) { argument = bytes.readUInt32BE(at); at += 4; }
  else if (info === 27) { argument = Number(bytes.readBigUInt64BE(at)); at += 8; }
  else if (info > 27) throw new Error(`indefinite or reserved CBOR item at ${offset}`);
  switch (major) {
    case 0: case 1: case 7: return at;
    case 2: case 3: return at + argument;
    case 4: for (let index = 0; index < argument; index++) at = cborEnd(bytes, at); return at;
    case 5: for (let index = 0; index < 2 * argument; index++) at = cborEnd(bytes, at); return at;
    default: return cborEnd(bytes, at);
  }
}
function cborHead(major, value) {
  if (value < 24) return Buffer.from([(major << 5) | value]);
  if (value < 0x100) return Buffer.from([(major << 5) | 24, value]);
  if (value < 0x10000) { const b = Buffer.alloc(3); b[0] = (major << 5) | 25; b.writeUInt16BE(value, 1); return b; }
  if (value < 0x100000000) { const b = Buffer.alloc(5); b[0] = (major << 5) | 26; b.writeUInt32BE(value, 1); return b; }
  const b = Buffer.alloc(9); b[0] = (major << 5) | 27; b.writeBigUInt64BE(BigInt(value), 1); return b;
}
const cborText = text => Buffer.concat([cborHead(3, Buffer.byteLength(text)), Buffer.from(text)]);
// topLevel splits one response payload's root map into raw entries, without
// re-encoding any value; rejoining them unchanged reproduces the payload.
function topLevel(payload) {
  const initial = payload[0];
  if (initial >> 5 !== 5) throw new Error("response payload is not a map");
  const headEnd = 1 + ({ 24: 1, 25: 2, 26: 4, 27: 8 }[initial & 0x1f] ?? 0);
  const entries = [];
  let at = headEnd;
  while (at < payload.length) {
    const keyEnd = cborEnd(payload, at);
    const valueEnd = cborEnd(payload, keyEnd);
    const keyBytes = payload.subarray(at, keyEnd);
    entries.push({ keyBytes, key: decodeText(keyBytes), value: payload.subarray(keyEnd, valueEnd) });
    at = valueEnd;
  }
  return { head: payload.subarray(0, headEnd), entries };
}
function decodeText(item) {
  const info = item[0] & 0x1f;
  const skip = info < 24 ? 1 : 1 + ({ 24: 1, 25: 2, 26: 4, 27: 8 }[info]);
  return item.subarray(skip).toString("utf8");
}
function decodeUint(item) {
  const info = item[0] & 0x1f;
  if (info < 24) return info;
  if (info === 24) return item[1];
  if (info === 25) return item.readUInt16BE(1);
  if (info === 26) return item.readUInt32BE(1);
  return Number(item.readBigUInt64BE(1));
}
// rewrite replaces one root value of a response frame, leaving every other
// byte as the producer wrote it. Undefined when the key is absent.
function rewrite(responseFrame, key, change) {
  const payload = responseFrame.subarray(4);
  const { head, entries } = topLevel(payload);
  const rejoined = Buffer.concat([head, ...entries.flatMap(entry => [entry.keyBytes, entry.value])]);
  if (!rejoined.equals(payload)) throw new Error("response map did not split losslessly");
  const entry = entries.find(candidate => candidate.key === key);
  if (!entry) return undefined;
  entry.value = change(entry.value);
  return frame(Buffer.concat([head, ...entries.flatMap(item => [item.keyBytes, item.value])]));
}

// Faults are applied by the host to the producer's answer before the checker
// copies it. `when` selects the exchange; `apply` returns the forged frame.
const FAULTS = {
  truncated: { when: index => index === 2, apply: response => response.subarray(0, response.length - 1) },
  trailing: { when: index => index === 2, apply: response => Buffer.concat([response, Buffer.from([0])]) },
  garbage: { when: index => index === 2, apply: () => frame(Buffer.alloc(8, 0xff)) },
  "wrong-request": {
    when: index => index === 2,
    apply: response => rewrite(response, "requestId", value => cborHead(0, decodeUint(value) + 1)),
  },
  "wrong-generation": {
    when: index => index === 2,
    apply: response => rewrite(response, "generation", value => cborHead(0, decodeUint(value) + 1)),
  },
  "wrong-project": {
    when: index => index === 2,
    apply: response => rewrite(response, "projectId", value => cborText(`${decodeText(value)}x`)),
  },
  // The first answer that carries a state token; it is checked when it is
  // next presented.
  "wrong-token": {
    when: (index, response) => topLevel(response.subarray(4)).entries.some(entry => entry.key === "stateToken"),
    apply: response => rewrite(response, "stateToken", value => cborText(`${decodeText(value)}x`)),
  },
  // The exchange is answered by a different program the same reactor owns.
  "wrong-program": { when: index => index === 2, route: "decoy" },
};

// Every program each producer instance issued and released, through any
// path: a check's imports, or the harness's own direct calls (the W5 decoy).
const ledgers = new WeakMap();
function ledgerOf(producer) {
  if (!ledgers.has(producer)) ledgers.set(producer, { opened: new Set(), closed: new Set() });
  return ledgers.get(producer);
}
const activePrograms = producer => {
  const ledger = ledgerOf(producer);
  return [...ledger.opened].filter(handle => !ledger.closed.has(handle));
};

function ownedHost(producer, record, fault) {
  const ledger = ledgerOf(producer);
  const results = new Map();
  const opened = new Set();
  const closed = new Set();
  let checkerMemory;
  let exchanges = 0;
  let faulted = false;
  const view = () => new Uint8Array(checkerMemory.buffer);
  const readChecker = (pointer, length) => {
    if (pointer + length > checkerMemory.buffer.byteLength) throw new Error("checker pointer out of bounds");
    return Buffer.from(view().slice(pointer, pointer + length));
  };
  const call = (operation, input, ...rest) => {
    const exports = producer.exports;
    if (input) {
      const pointer = exports.allocate_input(input.length);
      new Uint8Array(exports.memory.buffer, pointer, input.length).set(input);
    }
    const status = exports[operation](...rest);
    const output = Buffer.from(new Uint8Array(exports.memory.buffer, exports.output_pointer(), exports.output_length()));
    return { status, output };
  };
  const keep = (handle, status, output) => {
    results.set(handle, output);
    return status;
  };
  const imports = {
    open(pointer, length, handleOut) {
      const envelope = readChecker(pointer, length);
      const { status, output } = call("typefacts_open", envelope);
      record({ op: "open", status, envelope, output });
      if (status !== 0) return keep(0, status, output);
      const handle = output.readUInt32LE(0);
      opened.add(handle);
      ledger.opened.add(handle);
      if (handleOut + 4 > checkerMemory.buffer.byteLength) throw new Error("handle pointer out of bounds");
      new DataView(checkerMemory.buffer).setUint32(handleOut, handle, true);
      return 0;
    },
    handshake(handle) {
      const { status, output } = call("typefacts_handshake");
      const forged = fault?.handshake ? Buffer.concat([output.subarray(0, output.length - 1), Buffer.from([output[output.length - 1] ^ 1])]) : output;
      record({ op: "handshake", handle, status, output: forged });
      return keep(handle, status, forged);
    },
    exchange(handle, pointer, length) {
      const request = readChecker(pointer, length);
      exchanges++;
      let target = handle;
      if (fault?.route === "decoy" && !faulted && fault.when(exchanges)) {
        target = fault.decoy;
        faulted = true;
      }
      let { status, output } = call("typefacts_lifecycle", request, target);
      if (status === 0 && fault?.apply && !faulted && fault.when(exchanges, output)) {
        output = fault.apply(output);
        faulted = true;
      }
      record({ op: "exchange", handle, target, status, request, output });
      return keep(handle, status, output);
    },
    result_length(handle) {
      return results.get(handle)?.length ?? 0;
    },
    copy_result(handle, destination, capacity) {
      const result = results.get(handle);
      if (!result || result.length > capacity || destination + result.length > checkerMemory.buffer.byteLength) {
        results.delete(handle);
        return 1;
      }
      view().set(result, destination);
      results.delete(handle);
      return 0;
    },
    close(handle) {
      const { status, output } = call("typefacts_close", undefined, handle);
      record({ op: "close", handle, status, output });
      results.delete(handle);
      closed.add(handle);
      if (status === 0) ledger.closed.add(handle);
      return status;
    },
  };
  return {
    imports,
    bind: memory => { checkerMemory = memory; },
    state: () => ({ opened: [...opened], closed: [...closed], exchanges, faulted, pendingResults: [...results.keys()] }),
  };
}

function instantiate(module, fs, extra = {}) {
  const wasi = new WASI({ version: "preview1", fs, preopens: { "/": "/" }, env: {} });
  const instance = new WebAssembly.Instance(module, { ...wasi.getImportObject(), ...extra });
  wasi.initialize(instance);
  return instance;
}

// runOwned runs one check through both reactors and keeps every frame.
function runOwned(tree, request, { label = "wasi", omit, fault, instances } = {}) {
  const directory = join(tree.caseRoot, label);
  mkdirSync(join(directory, "frames"), { recursive: true });
  const fs = instances?.fs ?? mount(tree, omit);
  const producer = instances?.producer ?? instantiate(producerModule, fs);
  const frames = [];
  const record = entry => {
    const index = String(frames.length).padStart(3, "0");
    const files = {};
    for (const key of ["envelope", "request", "output"]) {
      if (!entry[key]) continue;
      const file = `${index}-${entry.op}-${key}.bin`;
      writeFileSync(join(directory, "frames", file), entry[key]);
      files[key] = { file, sha256: sha256(entry[key]), bytes: entry[key].length };
    }
    frames.push({ op: entry.op, handle: entry.handle, target: entry.target, status: entry.status, ...files });
  };
  const host = ownedHost(producer, record, fault);
  const checker = instances?.checker ?? instantiate(checkerModule, fs, { solid_typefacts: host.imports });
  if (instances?.rebind) instances.rebind(host.imports);
  host.bind(checker.exports.memory);
  const input = Buffer.from(JSON.stringify(request));
  const pointer = checker.exports.allocate_input(input.length);
  new Uint8Array(checker.exports.memory.buffer, pointer, input.length).set(input);
  const status = checker.exports.run_owned_check();
  const output = Buffer.from(new Uint8Array(checker.exports.memory.buffer,
    checker.exports.output_pointer(), checker.exports.output_length()));
  writeFileSync(join(directory, "output.json"), output);
  writeFileSync(join(directory, "frames.json"), `${JSON.stringify(frames, null, 2)}\n`);
  const parsed = JSON.parse(output.toString("utf8"));
  return { status, parsed, outputSha256: sha256(output), frames, host: host.state(), producer, checker, fs };
}

// --- comparison ----------------------------------------------------------------
const findingRows = snapshot => (snapshot?.findings ?? []).map(finding => ({
  id: finding.id,
  rule: finding.rule,
  kind: finding.kind,
  path: finding.primaryLocation?.path,
  startByte: finding.primaryLocation?.startByte,
  endByte: finding.primaryLocation?.endByte,
}));
function compare(tree, native, wasi, extra = {}) {
  const record = {
    case: tree.name,
    native: { status: native.status, stdoutSha256: native.stdoutSha256, snapshotStatus: native.snapshot?.status, notes: native.notes },
    wasi: { status: wasi.status, outputSha256: wasi.outputSha256, snapshotStatus: wasi.parsed.snapshot?.status, notes: wasi.parsed.notes, host: wasi.host, frames: wasi.frames.length },
    snapshotEqual: JSON.stringify(native.snapshot) === JSON.stringify(wasi.parsed.snapshot),
    notesEqual: JSON.stringify(native.notes) === JSON.stringify(wasi.parsed.notes ?? []),
    findings: findingRows(native.snapshot),
    metrics: native.snapshot?.metrics,
    premises: native.snapshot?.premises,
    ...extra,
  };
  check(native.status === 0, `${tree.name}: native exited ${native.status}`);
  check(wasi.status === 0, `${tree.name}: reactor check failed: ${wasi.parsed.error}`);
  check(record.snapshotEqual, `${tree.name}: snapshots differ`);
  check(record.notesEqual, `${tree.name}: notes differ: ${JSON.stringify(record.native.notes)} vs ${JSON.stringify(wasi.parsed.notes)}`);
  check(JSON.stringify([...wasi.host.opened].sort()) === JSON.stringify([...wasi.host.closed].sort()),
    `${tree.name}: handles opened ${wasi.host.opened} closed ${wasi.host.closed}`);
  check(wasi.host.pendingResults.length === 0 && activePrograms(wasi.producer).length === 0,
    `${tree.name}: results ${wasi.host.pendingResults} or programs ${activePrograms(wasi.producer)} left behind`);
  records.push(record);
  return record;
}
// compareRefusal pins a check both sides refuse: native exits 2 with one
// stderr line, the reactor returns status 1 with the same message, and
// neither produces a snapshot.
function compareRefusal(tree, native, wasi, extra = {}) {
  const record = {
    case: tree.name,
    native: { status: native.status, notes: native.notes },
    wasi: { status: wasi.status, error: wasi.parsed.error, host: wasi.host, frames: wasi.frames.map(({ op, status }) => ({ op, status })) },
    ...extra,
  };
  record.messageEqual = JSON.stringify(native.notes) === JSON.stringify([`solid-checker-rust: ${wasi.parsed.error}`]);
  check(native.status === 2 && native.snapshot === undefined, `${tree.name}: native exited ${native.status}`);
  check(wasi.status === 1 && wasi.parsed.snapshot === undefined, `${tree.name}: reactor status ${wasi.status}`);
  check(wasi.host.opened.every(handle => wasi.host.closed.includes(handle)), `${tree.name}: handles left open ${JSON.stringify(wasi.host)}`);
  check(wasi.host.pendingResults.length === 0 && activePrograms(wasi.producer).length === 0,
    `${tree.name}: results ${wasi.host.pendingResults} or programs ${activePrograms(wasi.producer)} left behind`);
  records.push(record);
  return record;
}
function expectRows(record, expected) {
  const actual = record.findings.map(({ id, kind, path, startByte, endByte }) =>
    `${id} ${kind} ${relative(join(out, "runs", record.case, "project"), path ?? "")}:${startByte}-${endByte}`);
  check(JSON.stringify(actual) === JSON.stringify(expected), `${record.case}: findings ${JSON.stringify(actual)}`);
}
const wanted = name => !only || only.has(name);

// --- witness projects ------------------------------------------------------------
// W1/W2 use the audited Solid 2 install the tsc oracle provisions (real
// published typings and runtime), with the oracle's strict compiler options,
// and source text taken verbatim from oracle cases where TypeScript is silent
// and the checker reports.
const oracleInstall = join(repositoryRoot, "rust/target/tsc-oracle/v2/node_modules");
const oracleCases = JSON.parse(readFileSync(join(repositoryRoot, "fixtures/tsc-oracle/rule-cases.json"), "utf8")).cases;
const oracleCode = rule => {
  const found = oracleCases.find(item => item.rule === rule && item.dialect === "v2" && item.expect === "silent" && item.checker === "reports");
  if (!found) throw new Error(`no oracle keystone for ${rule}`);
  return found.code.endsWith("\n") ? found.code : `${found.code}\n`;
};
function typedProject(files) {
  return root => {
    mkdirSync(root, { recursive: true });
    cpSync(oracleInstall, join(root, "node_modules"), { recursive: true, dereference: true });
    writeFileSync(join(root, "tsconfig.json"), `${JSON.stringify({
      compilerOptions: {
        target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve",
        jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM"], types: [], skipLibCheck: true,
        allowJs: false, baseUrl: ".", strict: true, noEmit: true,
      },
      files: Object.keys(files),
    }, null, 2)}\n`);
    for (const [name, text] of Object.entries(files)) writeFileSync(join(root, name), text);
  };
}
// tsc over the frozen tree, with the repository's TypeScript: the witness is
// valid only where TypeScript itself is silent.
const tscPath = join(repositoryRoot, "packages/cli/node_modules/typescript/bin/tsc");
function tsc(tree) {
  const result = spawnSync("node", [tscPath, "--noEmit", "-p", join(tree.root, "tsconfig.json")], { encoding: "utf8" });
  writeFileSync(join(tree.caseRoot, "tsc.out"), result.stdout + result.stderr);
  return { status: result.status, output: (result.stdout + result.stderr).trim() };
}

const w1Files = {
  "Gate.tsx": oracleCode("components-return-once"),
  "Row.tsx": oracleCode("no-destructure"),
  "Hello.tsx": "export function Hello() {\n  return <div>hello</div>;\n}\n",
};

function standard(name, source, expected, { mutate, validateTsc = false, extra = {} } = {}) {
  if (!wanted(name)) return undefined;
  const tree = freeze(name, source, mutate);
  const request = caseRequest(tree);
  const typescript = validateTsc ? tsc(tree) : undefined;
  if (typescript) check(typescript.status === 0, `${name}: tsc reports: ${typescript.output}`);
  const native = runNative(tree, request);
  const wasi = runOwned(tree, request.request);
  const record = compare(tree, native, wasi, { request: request.request, argv: request.argv, tsc: typescript, ...extra });
  if (expected) {
    check(native.snapshot?.status === expected.status, `${name}: status ${native.snapshot?.status}, expected ${expected.status}`);
    if (expected.rows) expectRows(record, expected.rows);
  }
  return { tree, request, native, wasi, record };
}

// W1: positive, a proven violation and an open exported-props obligation on
// real typings, at the oracle keystones' own sites.
const w1 = standard("w1-real-typings-violation", typedProject(w1Files), {
  status: "violation",
  rows: ["SC1004 violation Gate.tsx:109-116", "SC1003 uncertifiable Row.tsx:55-63"],
}, { validateTsc: true });

// W2: clean on the same install, then a nonzero uncertifiable fixture.
standard("w2-real-typings-clean", typedProject({ "Hello.tsx": w1Files["Hello.tsx"] }), { status: "certified", rows: [] }, { validateTsc: true });
const w2u = standard("w2-uncertifiable", join(repositoryRoot, "fixtures/reactive-ir/package-contract-absent"), { status: "uncertifiable" });
if (w2u) check(w2u.record.findings.length > 0 && w2u.record.findings.every(row => row.kind === "uncertifiable"), "w2-uncertifiable: rows");

// W3: installed JS dependency source analyzed under the ADR 0297 selection.
const w3 = standard("w3-dependency-source-program", join(repositoryRoot, "fixtures/reactive-ir/dependency-source-program"), { status: "violation" });
if (w3) check(w3.record.findings.some(row => row.id === "SC1001"), "w3: no SC1001 through the dependency source");
const w3js = standard("w3-visible-js", join(repositoryRoot, "fixtures/reactive-ir/dependency-source-integrity-r2-visible-js"), { status: "uncertifiable" });
if (w3js) check(w3js.record.findings.some(row => row.id === "SC1001"), "w3-visible-js: no SC1001 through the visible JS");

// P1 (CODE review r1): an import a contract tier covers keeps its obligation
// when no contract is consumed. A local reactive callback goes to an
// installed package that names no Solid dependency and whose implementation
// nothing analyzes: uncertifiable with the obligations open, no violation.
const disabledNote = "package contracts are not consumed in this check (--no-package-contracts)";
const p1 = standard("p1-unknown-callback", join(repositoryRoot, "fixtures/reactive-ir/package-unknown-callback-consumer"), {
  status: "uncertifiable",
  rows: ["SC9005 uncertifiable App.ts:41-98"],
});
if (p1) {
  const metrics = p1.native.snapshot.metrics;
  check(metrics.proofObligations === 2 && metrics.unresolvedObligations === 2, `p1: metrics ${JSON.stringify(metrics)}`);
  check(p1.native.snapshot.findings[0].evidence.some(step => step.message.startsWith(disabledNote)), "p1: no Disabled note");
}
// The negative beside it: a specifier `paths` maps onto project source stays
// project source; only the import resolving into the cataloged install is open.
standard("p1-paths-shadow", join(repositoryRoot, "fixtures/reactive-ir/package-contract-paths-shadow"), {
  status: "uncertifiable",
  rows: ["SC9005 uncertifiable App.ts:45-107"],
});
// Install shapes: an ambiently typed install and a subpath under an unnamed
// manifest stay open; the `@types`-redirected specifier, which the catalog
// does not name, raises nothing.
standard("p1-install-shapes", join(repositoryRoot, "fixtures/reactive-ir/package-contract-install-shapes"), {
  status: "uncertifiable",
  rows: ["SC9005 uncertifiable App.ts:0-57", "SC9005 uncertifiable App.ts:122-181"],
});

// Claim C through the actual owner on both hosts: every supported spelling of
// the P1 catalog importer keeps its canonical key and both obligations, and an
// unestablishable discovery or covered installation refuses on both sides.
function respellImporter(root, spelling) {
  const path = join(root, ".solid-checker/accepted-contracts.json");
  const catalog = JSON.parse(readFileSync(path, "utf8"));
  for (const entry of catalog.contracts) entry.import.importer = spelling.replaceAll("{project}", root);
  writeFileSync(path, `${JSON.stringify(catalog, null, 2)}\n`);
}
for (const [variant, spelling] of [
  ["absolute", "{project}/App.ts"],
  ["dotdot", "{project}/node_modules/../App.ts"],
  ["symlink", "alias/App.ts"],
]) {
  const name = `p1-key-${variant}`;
  const result = standard(name, join(repositoryRoot, "fixtures/reactive-ir/package-unknown-callback-consumer"), {
    status: "uncertifiable",
    rows: ["SC9005 uncertifiable App.ts:41-98"],
  }, {
    mutate: root => {
      symlinkSync(".", join(root, "alias"));
      respellImporter(root, spelling);
    },
  });
  if (result) {
    const metrics = result.native.snapshot.metrics;
    check(metrics.proofObligations === 2 && metrics.unresolvedObligations === 2, `${name}: metrics ${JSON.stringify(metrics)}`);
    check(result.native.snapshot.findings[0].evidence.some(step => step.message.startsWith(disabledNote)), `${name}: no Disabled note`);
  }
}
function refusal(name, source, mutate, reason, { producerRefusesFirst = false } = {}) {
  if (!wanted(name)) return;
  const tree = freeze(name, source, mutate);
  const request = caseRequest(tree);
  const record = compareRefusal(tree, runNative(tree, request), runOwned(tree, request.request), {
    producerRefusesFirst,
  });
  check(record.native.notes.some(line => line.includes(reason)), `${name}: native refusal ${record.native.notes}`);
  if (producerRefusesFirst) {
    // Recorded, not normalized: the WASI producer's own project open refuses
    // this tree before the checker reaches its provenance refusal. Both sides
    // still refuse with no snapshot.
    check(String(record.wasi.error ?? "").startsWith("open TS-Go project"), `${name}: WASI refusal ${record.wasi.error}`);
  } else {
    check(record.messageEqual, `${name}: refusals differ: ${JSON.stringify(record)}`);
    check(String(record.wasi.error ?? "").includes(reason), `${name}: refusal ${record.wasi.error}`);
  }
}
refusal("refuse-dangling-catalog-root", join(repositoryRoot, "fixtures/reactive-ir/package-unknown-callback-consumer"), root => {
  rmSync(join(root, ".solid-checker"), { recursive: true });
  symlinkSync("missing-target", join(root, ".solid-checker"));
}, "dangling symlink", { producerRefusesFirst: true });
refusal("refuse-dangling-plain-catalog", join(repositoryRoot, "fixtures/reactive-ir/package-unknown-callback-consumer"), root => {
  rmSync(join(root, ".solid-checker/accepted-contracts.json"));
  symlinkSync("missing-target", join(root, ".solid-checker/accepted-contracts.json"));
}, "dangling symlink");
refusal("refuse-dangling-installation", join(repositoryRoot, "fixtures/reactive-ir/package-contract-install-shapes"), root => {
  rmSync(join(root, "node_modules/ambient-package"), { recursive: true });
  symlinkSync("missing-target", join(root, "node_modules/ambient-package"));
}, "dangling symlink");
// The covered package genuinely absent: answered, with no requirement there.
standard("install-absent", join(repositoryRoot, "fixtures/reactive-ir/package-contract-install-shapes"), {
  status: "uncertifiable",
  rows: ["SC9005 uncertifiable App.ts:122-181"],
}, { mutate: root => rmSync(join(root, "node_modules/ambient-package"), { recursive: true }) });

// W4: one input missing identically on disk and in the mount, against the
// intact result.
//
// (a) A module the dependency's selected `solid` source imports goes; its
// declarations stay, so TypeScript's answer over the project does not change,
// and both sides refuse the dependency program with the same message.
if (wanted("w4-omission") && w3) {
  const omittedRelative = "node_modules/dep-hooks/src/read.js";
  const tree = freeze("w4-omission", join(repositoryRoot, "fixtures/reactive-ir/dependency-source-program"), root => {
    if (!existsSync(join(root, omittedRelative))) throw new Error(`w4: ${omittedRelative} is not in the fixture`);
    rmSync(join(root, omittedRelative));
  });
  const request = caseRequest(tree);
  const native = runNative(tree, request);
  const wasi = runOwned(tree, request.request);
  const relativeTsc = (target, result) => result.output.replaceAll(relative(repositoryRoot, target.root), "<root>");
  const typescript = { intact: tsc(w3.tree), omitted: tsc(tree) };
  const record = compareRefusal(tree, native, wasi, { omitted: omittedRelative, tsc: typescript });
  check(record.messageEqual, `w4: refusals differ: ${JSON.stringify(native.notes)} vs ${wasi.parsed.error}`);
  check(/imports "\.\/read\.js", which resolves to no source file$/.test(wasi.parsed.error ?? ""), `w4: refusal ${wasi.parsed.error}`);
  check(relativeTsc(w3.tree, typescript.intact) === relativeTsc(tree, typescript.omitted), "w4: the omission changed what TypeScript reports");
}

// (b) A project source another one imports goes. TypeScript reports TS2307;
// the checker loses the omitted file's own finding and must not add one of its
// own for the missing module.
if (wanted("w4-project-omission") && w1) {
  const files = { ...w1Files, "Use.tsx": 'import { Gate } from "./Gate";\nexport function Use() {\n  return <Gate />;\n}\n' };
  const intact = standard("w4-project-intact", typedProject(files), {
    status: "violation",
    rows: ["SC1004 violation Gate.tsx:109-116", "SC1003 uncertifiable Row.tsx:55-63"],
  }, { validateTsc: true });
  const omitted = standard("w4-project-omitted", typedProject(files), {
    status: "uncertifiable",
    rows: ["SC1003 uncertifiable Row.tsx:55-63"],
  }, {
    // Out of the root files too, so the only thing missing is Use's import.
    mutate: root => {
      rmSync(join(root, "Gate.tsx"));
      const config = JSON.parse(readFileSync(join(root, "tsconfig.json"), "utf8"));
      config.files = config.files.filter(name => name !== "Gate.tsx");
      writeFileSync(join(root, "tsconfig.json"), `${JSON.stringify(config, null, 2)}\n`);
    },
  });
  if (omitted) {
    const typescript = tsc(omitted.tree);
    omitted.record.tsc = typescript;
    check(/Use\.tsx\(1,22\): error TS2307/.test(typescript.output), `w4b: tsc ${typescript.output}`);
    check(intact && !omitted.record.findings.some(row => row.path?.endsWith("/Use.tsx")), "w4b: a finding at the missing import");
  }
}

// W5: forged producer answers and a decoy program. Every one refuses with no
// snapshot; the same instances then answer a clean check identically.
if (wanted("w5-faults") && w3) {
  const tree = freeze("w5-faults", join(repositoryRoot, "fixtures/reactive-ir/dependency-source-program"));
  const request = caseRequest(tree);
  const reference = runNative(tree, request);
  const decoyTree = freeze("w5-decoy", typedProject({ "Hello.tsx": w1Files["Hello.tsx"] }));
  const fs = mount(tree);
  // The decoy lives in the same mount, so the producer can open it.
  fs.mkdirSync(decoyTree.root, { recursive: true });
  for (const [path, entry] of decoyTree.entries) {
    if (entry.directory) fs.mkdirSync(path, { recursive: true });
    else if (entry.link) fs.symlinkSync(entry.link, path);
    else fs.writeFileSync(path, entry.bytes);
  }
  const producer = instantiate(producerModule, fs);
  let imports = {};
  const forwarding = Object.fromEntries(["open", "handshake", "exchange", "result_length", "copy_result", "close"]
    .map(name => [name, (...rest) => imports[name](...rest)]));
  const checker = instantiate(checkerModule, fs, { solid_typefacts: forwarding });
  const instances = { fs, producer, checker, rebind: next => { imports = next; } };
  const results = {};
  const call = (operation, input, ...rest) => {
    const exports = producer.exports;
    if (input) {
      const pointer = exports.allocate_input(input.length);
      new Uint8Array(exports.memory.buffer, pointer, input.length).set(input);
    }
    const status = exports[operation](...rest);
    return { status, output: Buffer.from(new Uint8Array(exports.memory.buffer, exports.output_pointer(), exports.output_length())) };
  };
  for (const [name, spec] of Object.entries(FAULTS)) {
    let fault = spec;
    // The decoy is opened through the producer's own ABI, outside any check,
    // and released whatever the check does.
    let decoy;
    if (spec.route === "decoy") {
      const opened = call("typefacts_open", cborOpenEnvelope(join(decoyTree.root, "tsconfig.json")));
      if (opened.status !== 0) throw new Error(`decoy open failed: ${opened.output}`);
      decoy = opened.output.readUInt32LE(0);
      ledgerOf(producer).opened.add(decoy);
      fault = { ...spec, decoy };
    }
    let faulted;
    let decoyClose;
    try {
      faulted = runOwned(tree, request.request, { label: `fault-${name}`, fault, instances });
    } finally {
      if (decoy !== undefined) {
        decoyClose = call("typefacts_close", undefined, decoy).status;
        if (decoyClose === 0) ledgerOf(producer).closed.add(decoy);
      }
    }
    const clean = runOwned(tree, request.request, { label: `after-${name}`, instances });
    results[name] = {
      status: faulted.status,
      error: faulted.parsed.error,
      snapshot: faulted.parsed.snapshot !== undefined,
      host: faulted.host,
      decoy: decoy === undefined ? undefined : { handle: decoy, closeStatus: decoyClose },
      afterStatus: clean.status,
      afterSnapshotEqual: JSON.stringify(clean.parsed.snapshot) === JSON.stringify(reference.snapshot),
      afterHost: clean.host,
      activeProgramsAfter: activePrograms(producer),
    };
    check(faulted.host.faulted, `w5/${name}: the fault was never applied`);
    check(faulted.status === 1 && faulted.parsed.snapshot === undefined, `w5/${name}: status ${faulted.status}, snapshot ${faulted.parsed.snapshot !== undefined}`);
    check(faulted.host.opened.every(handle => faulted.host.closed.includes(handle)), `w5/${name}: handles left open ${JSON.stringify(faulted.host)}`);
    check(faulted.host.pendingResults.length === 0 && clean.host.pendingResults.length === 0,
      `w5/${name}: results left behind`);
    check(decoy === undefined || decoyClose === 0, `w5/${name}: the decoy did not close (${decoyClose})`);
    check(clean.status === 0 && results[name].afterSnapshotEqual, `w5/${name}: the clean check afterwards differs`);
    // Every program this producer ever issued -- the checks', the decoy --
    // is released before the next fault.
    check(results[name].activeProgramsAfter.length === 0, `w5/${name}: programs ${results[name].activeProgramsAfter} still open`);
  }
  const ledger = ledgerOf(producer);
  records.push({
    case: "w5-faults",
    referenceStatus: reference.snapshot?.status,
    results,
    programs: { opened: [...ledger.opened], closed: [...ledger.closed], active: activePrograms(producer) },
  });
}

// The open envelope, encoded exactly as typefacts::owned_open_envelope writes
// it for a program without a configuration overlay.
function cborOpenEnvelope(projectId) {
  return Buffer.concat([cborHead(5, 1), cborText("projectId"), cborText(projectId)]);
}

// W6: changed frozen trees through fresh checks: the W1 project with Gate
// returning once is a different, decisive answer on both sides.
if (wanted("w6-changed") && w1) {
  const changed = standard("w6-changed-fresh", typedProject({
    ...w1Files,
    "Gate.tsx": w1Files["Gate.tsx"]
      .replace('import { createSignal } from "solid-js";', 'import { Show, createSignal } from "solid-js";')
      .replace(
        "  if (ready()) return <div>on</div>;\n  return <span>off</span>;\n",
        "  return <Show when={ready()} fallback={<span>off</span>}><div>on</div></Show>;\n",
      ),
  }), undefined, { validateTsc: true });
  if (changed) {
    check(changed.record.findings.length === w1.record.findings.length - 1,
      `w6: ${changed.record.findings.length} findings after the change, ${w1.record.findings.length} before`);
    const gate = path => path?.endsWith("/Gate.tsx");
    check(w1.record.findings.some(row => gate(row.path)) && !changed.record.findings.some(row => gate(row.path)),
      "w6: the Gate finding did not move");
  }
}

// W6 (update): the Rust Session's own update frames, recorded over the real
// native producer by the typefacts bridge test, replayed into the producer
// reactor over the same tree: every answer is byte-identical.
if (wanted("w6-update") && options["update-frames"]) {
  const directory = resolve(options["update-frames"]);
  const recording = JSON.parse(readFileSync(join(directory, "frames.json"), "utf8"));
  const tree = { name: "w6-update-replay", caseRoot: join(out, "runs", "w6-update-replay"), root: recording.root };
  mkdirSync(tree.caseRoot, { recursive: true });
  tree.entries = snapshot(tree.root);
  tree.manifest = manifest(tree.entries);
  writeFileSync(join(tree.caseRoot, "mount-manifest.json"), `${JSON.stringify(tree.manifest, null, 2)}\n`);
  const producer = instantiate(producerModule, mount(tree));
  const exports = producer.exports;
  const call = (operation, input, ...rest) => {
    if (input) {
      const pointer = exports.allocate_input(input.length);
      new Uint8Array(exports.memory.buffer, pointer, input.length).set(input);
    }
    const status = exports[operation](...rest);
    return { status, output: Buffer.from(new Uint8Array(exports.memory.buffer, exports.output_pointer(), exports.output_length())) };
  };
  const handshake = call("typefacts_handshake");
  const opened = call("typefacts_open", cborOpenEnvelope(recording.projectId));
  check(opened.status === 0, `w6-update: open failed ${opened.output}`);
  const handle = opened.output.readUInt32LE(0);
  const replay = {
    handshakeEqual: handshake.output.equals(readFileSync(join(directory, recording.handshake))),
    exchanges: [],
  };
  // Analyze answers carry the producer's own wall-clock timings, which no two
  // runs share; everything else in every answer must be byte-identical.
  const withoutTimings = response => {
    const { head, entries } = topLevel(response.subarray(4));
    const kept = entries.filter(entry => entry.key !== "timings");
    if (kept.length === entries.length) return response;
    return frame(Buffer.concat([
      Buffer.from(cborHead(5, kept.length)), ...kept.flatMap(entry => [entry.keyBytes, entry.value]),
    ]));
  };
  for (const [index, exchange] of recording.exchanges.entries()) {
    const answered = call("typefacts_lifecycle", readFileSync(join(directory, exchange.request)), handle);
    writeFileSync(join(tree.caseRoot, `${String(index).padStart(3, "0")}-${exchange.operation}-wasi.bin`), answered.output);
    const expected = readFileSync(join(directory, exchange.response));
    replay.exchanges.push({
      operation: exchange.operation,
      status: answered.status,
      equal: answered.output.equals(expected),
      equalWithoutTimings: withoutTimings(answered.output).equals(withoutTimings(expected)),
      bytes: expected.length,
      sha256: sha256(expected),
    });
  }
  check(replay.handshakeEqual, "w6-update: handshake differs");
  check(replay.exchanges.length > 0 && replay.exchanges.every(item => item.status === 0 && item.equalWithoutTimings),
    "w6-update: a replayed answer differs beyond its timings");
  check(replay.exchanges.every(item => item.equal || item.operation === "analyze"), "w6-update: a non-analyze answer differs");
  check(replay.exchanges.some(item => item.operation === "update"), "w6-update: the recording has no update");
  call("typefacts_close", undefined, handle);
  records.push({ case: "w6-update-replay", root: recording.root, replay });
}

// W7: a missing project, a project the producer cannot open, and a handshake
// that does not match each refuse with no snapshot and leave no program open.
if (wanted("w7-open") && w1) {
  const missing = freeze("w7-missing-project", typedProject({ "Hello.tsx": w1Files["Hello.tsx"] }), root => {
    rmSync(join(root, "tsconfig.json"));
  });
  const missingRequest = caseRequest(missing);
  const missingRecord = compareRefusal(missing, runNative(missing, missingRequest), runOwned(missing, missingRequest.request));
  check(missingRecord.wasi.frames.length === 0, "w7: a missing project reached the producer");
  // The same ENOENT; WASI numbers it 44 where the host numbers it 2.
  check(JSON.stringify(missingRecord.native.notes) === JSON.stringify(["solid-checker-rust: No such file or directory (os error 2)"]) &&
    missingRecord.wasi.error === "No such file or directory (os error 44)", `w7: missing project ${JSON.stringify(missingRecord)}`);
  const malformed = freeze("w7-open-failure", typedProject({ "Hello.tsx": w1Files["Hello.tsx"] }), root => {
    writeFileSync(join(root, "tsconfig.json"), '{ "compilerOptions": { "strict": true }, "files": 7 }\n');
  });
  const malformedRequest = caseRequest(malformed);
  const failed = runOwned(malformed, malformedRequest.request);
  const failedRecord = compareRefusal(malformed, runNative(malformed, malformedRequest), failed);
  check(failed.host.opened.length === 0 && failed.frames.some(item => item.op === "open" && item.status !== 0),
    `w7: open failure ${JSON.stringify(failedRecord.wasi)}`);
  // Natively the producer process dies opening the project and the process
  // carrier reports the lost pipe; the owned reactor returns the open error.
  check(failedRecord.native.notes.includes(`solid-typefacts: ${failed.parsed.error}`),
    `w7: open failure messages ${JSON.stringify(failedRecord)}`);
  const handshake = runOwned(w1.tree, caseRequest(w1.tree).request, { label: "w7-bad-handshake", fault: { handshake: true } });
  const record = {
    case: "w7-bad-handshake",
    badHandshake: { status: handshake.status, error: handshake.parsed.error, host: handshake.host },
  };
  check(handshake.status === 1 && handshake.parsed.snapshot === undefined &&
    handshake.host.opened.length > 0 && handshake.host.opened.every(handle => handshake.host.closed.includes(handle)),
    `w7: bad handshake ${JSON.stringify(record.badHandshake)}`);
  records.push(record);
}

const summary = {
  question: "Does the shared ordinary check, with package contracts disabled, answer identically natively and through the owned Type Facts reactor over the same mounted bytes?",
  repositoryRoot,
  out,
  images,
  records,
  failures,
};
writeFileSync(join(out, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
console.log(JSON.stringify({ out, images, records, failures }, null, 2));
if (failures.length !== 0) process.exit(1);
