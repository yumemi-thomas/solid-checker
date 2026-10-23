// The parity gate's driver: runs inside the isolated install, reaches the
// package only by its specifier, and writes every raw answer for the gate to
// compare. Ordinary cases use only the public `owned.mjs`; negative controls
// attach the package's own client to the gate's fault worker.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { MessageChannel, Worker } from "node:worker_threads";
import { withoutTimings, cborOpenEnvelope } from "./cbor.mjs";

const plan = JSON.parse(readFileSync(process.argv[2], "utf8"));
const owned = await import("solid-checker-wasm/owned.mjs");
const { attach } = await import("solid-checker-wasm/owned-client.mjs");
const { loadArtifacts } = await import("solid-checker-wasm/owned-artifacts.mjs");
const { materializeMount, mountBuffers, validateMount } = await import("solid-checker-wasm/owned-mount.mjs");
const { identityOf } = await import("solid-checker-wasm/owned-snapshot.mjs");

const results = {
  engine: process.versions.bun ? `bun ${process.versions.bun}` : `node ${process.version}`,
  resolved: {
    owned: import.meta.resolve("solid-checker-wasm/owned.mjs"),
    wasmUtil: import.meta.resolve("@tybys/wasm-util"),
    fs: import.meta.resolve("@napi-rs/wasm-runtime/fs"),
  },
  cases: {},
};
const record = (name, value) => {
  if (name in results.cases) throw new Error(`case ${name} recorded twice`);
  results.cases[name] = value;
};
const plain = result => result.ok
  ? { ok: true, snapshot: result.snapshot, notes: result.notes, json: result.json, transport: result.transport }
  : { ok: false, error: result.error, json: result.json, transport: result.transport };
const rejected = error => ({ rejected: true, name: error?.name, message: error?.message ?? String(error) });
const settle = promise => promise.then(plain, rejected);
const attempt = async action => {
  try {
    return { value: await action() };
  } catch (error) {
    return rejected(error);
  }
};
const waitFor = async (condition, what) => {
  const deadline = Date.now() + 30_000;
  while (!condition()) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise(resolve => setTimeout(resolve, 10));
  }
};

async function once(root, request) {
  const checker = await owned.createOwnedChecker({ mount: owned.mountDirectory(root) });
  try {
    return await settle(checker.check(request));
  } finally {
    await checker.close();
  }
}

// The same check with the tree read on demand (P5 S3b): the gate requires
// exactly the mount's answer.
async function onceDirectory(root, request) {
  const checker = await owned.createOwnedChecker({ directory: root });
  try {
    return await settle(checker.check(request));
  } finally {
    await checker.close();
  }
}

// The package's client over the gate's fault worker, never the shipped one.
async function faultChecker(mode, mount, extra = {}) {
  const channel = new MessageChannel();
  const records = [];
  channel.port1.on("message", message => records.push(message));
  const worker = new Worker(new URL("./fault-worker.mjs", import.meta.url), {
    workerData: { mode, side: channel.port2, ...extra },
    transferList: [channel.port2],
  });
  const exit = { code: undefined };
  const exited = new Promise(resolve => worker.once("exit", code => {
    exit.code = code;
    resolve(code);
  }));
  const { checker, producer } = await loadArtifacts();
  const validated = validateMount(mount);
  const api = await attach(worker, { checker, producer, mount: validated, transfer: [checker, producer, ...mountBuffers(validated)] });
  const finish = async () => {
    await api.close();
    await exited;
    channel.port1.close();
  };
  return { checker: api, records, exited, exit, finish };
}

const w1 = plan.cases.w1;
const w2 = plan.cases.w2;

// The legacy witness's ordinary cases, each through a fresh public checker.
for (const item of plan.standard) record(item.name, await once(item.root, item.request));
for (const item of plan.standard) record(`directory:${item.name}`, await onceDirectory(item.root, item.request));

// W5: every forged answer, each followed by a clean check on the same pair.
{
  const mount = {
    root: plan.w5.mountRoot,
    entries: plan.w5.trees.flatMap(tree => [
      { kind: "directory", path: tree.directory },
      ...owned.mountDirectory(tree.directory).entries,
    ]),
  };
  const faults = await faultChecker("faults", mount, { decoyProjectId: plan.w5.decoyProjectId });
  const checks = [];
  for (let index = 0; index < plan.w5.checks; index++) checks.push(await settle(faults.checker.check(plan.w5.request)));
  await waitFor(() => faults.records.filter(item => item.type === "check").length === plan.w5.checks, "W5 records");
  await faults.finish();
  record("w5-faults", { checks, records: faults.records });
}

// W7: a forged handshake byte.
{
  const handshake = await faultChecker("handshake", owned.mountDirectory(w1.root));
  const result = await settle(handshake.checker.check(w1.request));
  await handshake.finish();
  record("w7-bad-handshake", { result, records: handshake.records });
}

// Reuse after an answer and after a refusal, on one public checker.
{
  const checker = await owned.createOwnedChecker({ mount: owned.mountDirectory(w1.root) });
  const first = await settle(checker.check(w1.request));
  const refusal = await settle(checker.check({ ...w1.request, projectId: plan.cases.missingProjectId }));
  const again = await settle(checker.check(w1.request));
  await checker.close();
  record("lifecycle-reuse", { first, refusal, again });
}

// Close with one check in flight and one queued.
{
  const checker = await owned.createOwnedChecker({ mount: owned.mountDirectory(w1.root) });
  const inFlight = settle(checker.check(w1.request));
  const queued = settle(checker.check(w1.request));
  const closing = checker.close();
  const sameClose = checker.close() === closing;
  await closing;
  record("lifecycle-close", {
    inFlight: await inFlight,
    queued: await queued,
    after: await settle(checker.check(w1.request)),
    sameClose,
  });
}

// Poison: a trap, the worker's exit, a leaked program, an uncaught error, and
// a result whose answer is invalid JSON, the wrong shape or an unknown status.
for (const mode of ["trap", "exit", "ledger", "error", "malformed-json", "malformed-shape", "malformed-status"]) {
  const poisoned = await faultChecker(mode, owned.mountDirectory(w1.root));
  const first = settle(poisoned.checker.check(w1.request));
  const queued = settle(poisoned.checker.check(w1.request));
  const value = { first: await first, queued: await queued, after: await settle(poisoned.checker.check(w1.request)) };
  await poisoned.exited;
  value.exitCode = poisoned.exit.code;
  await poisoned.finish();
  record(mode.startsWith("malformed-") ? mode : `poison-${mode}`, value);
}

// A destination one byte too small: refused and cleaned up, then reused.
{
  const short = await faultChecker("short-capacity", owned.mountDirectory(w1.root));
  const first = await settle(short.checker.check(w1.request));
  const after = await settle(short.checker.check(w1.request));
  await short.finish();
  record("short-capacity", { first, after });
}

// Corrupted package artifacts refuse to start.
for (const [name, directory] of Object.entries(plan.corruptPackages)) {
  record(name, await attempt(async () => {
    const corrupt = await import(new URL("owned.mjs", `file://${directory}/`).href);
    const checker = await corrupt.createOwnedChecker({ mount: corrupt.mountDirectory(w2.root) });
    await checker.close();
    return "started";
  }));
}

// Two checkers at once.
{
  const [a, b] = await Promise.all([once(w1.root, w1.request), once(w2.root, w2.request)]);
  record("concurrent", { a, b });
}

// Mounts that must refuse, and an option the public API does not have.
record("mount-outside-link", await attempt(() => owned.mountDirectory(plan.mounts.outsideLink)));
record("mount-fifo", await attempt(() => owned.mountDirectory(plan.mounts.fifo)));
const crafted = async mount => attempt(async () => {
  const checker = await owned.createOwnedChecker({ mount });
  await checker.close();
  return "started";
});
record("mount-non-normal", await crafted({ root: "/m", entries: [{ kind: "file", path: "/m/a/../b", bytes: "" }] }));
record("mount-duplicate", await crafted({ root: "/m", entries: [{ kind: "file", path: "/m/a", bytes: "" }, { kind: "file", path: "/m/a", bytes: "" }] }));
record("mount-undeclared-parent", await crafted({ root: "/m", entries: [{ kind: "file", path: "/m/x/y", bytes: "" }] }));
record("mount-unsupported-kind", await crafted({ root: "/m", entries: [{ kind: "fifo", path: "/m/p" }] }));
record("option-unknown", await attempt(async () => {
  const checker = await owned.createOwnedChecker({ mount: owned.mountDirectory(w2.root), adapters: {} });
  await checker.close();
  return "started";
}));

// Mount capture: every caller property is read once, and only what was read
// and validated is materialized. Each variant is compared with the plain
// mount of the same files.
{
  const tsconfig = '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n';
  const app = "export const answer = 42;\n";
  const plainEntries = () => [
    { kind: "file", path: "/m/tsconfig.json", bytes: tsconfig },
    { kind: "file", path: "/m/App.ts", bytes: app },
  ];
  const checkMemory = async (mount, projectIds) => {
    const checker = await owned.createOwnedChecker({ mount });
    try {
      const answers = [];
      for (const projectId of projectIds) answers.push(await settle(checker.check({ projectId })));
      return answers;
    } finally {
      await checker.close();
    }
  };
  const captured = mount => validateMount(mount).entries.map(entry =>
    [entry.kind, entry.path, entry.target ?? new TextDecoder().decode(entry.bytes)]);
  const [plain] = await checkMemory({ root: "/m", entries: plainEntries() }, ["/m/tsconfig.json"]);

  let pathReads = 0;
  const changingPath = () => {
    const entries = plainEntries();
    entries[0] = { kind: "file", get path() { return ++pathReads === 1 ? "/m/tsconfig.json" : "/outside/tsconfig.json"; }, bytes: tsconfig };
    return { root: "/m", entries };
  };
  const [pathInside, pathOutside] = await checkMemory(changingPath(), ["/m/tsconfig.json", "/outside/tsconfig.json"]);
  const pathCheckReads = pathReads;
  pathReads = 0;
  record("mount-changing-path", { plain, inside: pathInside, outside: pathOutside, reads: pathCheckReads, captured: captured(changingPath()), capturedReads: pathReads });

  let targetReads = 0;
  const withAlias = target => ({ root: "/m", entries: [...plainEntries(), { kind: "symlink", path: "/m/alias", ...target }] });
  const [aliasPlain] = await checkMemory(withAlias({ target: "." }), ["/m/alias/tsconfig.json"]);
  const changingTarget = () => withAlias({ get target() { return ++targetReads === 1 ? "." : "/outside"; } });
  const [aliasChanged] = await checkMemory(changingTarget(), ["/m/alias/tsconfig.json"]);
  const targetCheckReads = targetReads;
  targetReads = 0;
  record("mount-changing-target", { plain: aliasPlain, changed: aliasChanged, reads: targetCheckReads, captured: captured(changingTarget()), capturedReads: targetReads });

  const mutating = () => {
    const bytes = new TextEncoder().encode(app);
    const entries = [
      { kind: "file", path: "/m/tsconfig.json", bytes: tsconfig },
      { kind: "file", path: "/m/App.ts", bytes },
    ];
    entries.push({
      kind: "file",
      get path() {
        entries[0].path = "/outside/tsconfig.json";
        bytes.fill(0x21);
        entries.push({ kind: "file", path: "/outside/extra", bytes: "" });
        return "/m/late.ts";
      },
      bytes: "export {};\n",
    });
    return { root: "/m", entries };
  };
  const [mutationPlain] = await checkMemory({ root: "/m", entries: [...plainEntries(), { kind: "file", path: "/m/late.ts", bytes: "export {};\n" }] }, ["/m/tsconfig.json"]);
  const [mutationInside, mutationOutside] = await checkMemory(mutating(), ["/m/tsconfig.json", "/outside/tsconfig.json"]);
  record("mount-mutation-during-capture", { plain: mutationPlain, inside: mutationInside, outside: mutationOutside, captured: captured(mutating()) });

  const configBytes = new TextEncoder().encode(tsconfig);
  const copied = await owned.createOwnedChecker({ mount: { root: "/m", entries: [{ kind: "file", path: "/m/tsconfig.json", bytes: configBytes }, plainEntries()[1]] } });
  configBytes.fill(0x7b);
  const afterWrite = await settle(copied.check({ projectId: "/m/tsconfig.json" }));
  await copied.close();
  const recordBytes = new TextEncoder().encode(tsconfig);
  const copy = validateMount({ root: "/m", entries: [{ kind: "file", path: "/m/tsconfig.json", bytes: recordBytes }] });
  recordBytes.fill(0x7b);
  record("mount-byte-copy", { plain, afterWrite, captured: new TextDecoder().decode(copy.entries[0].bytes), callerBuffer: new TextDecoder().decode(recordBytes) });
}

// Project spellings, each against its own native run.
for (const spelling of plan.spellings) record(spelling.name, await once(spelling.root, spelling.request));
for (const spelling of plan.spellings) record(`directory:${spelling.name}`, await onceDirectory(spelling.root, spelling.request));

// S3a: the semantic fixture with a `../` dependency import, and the same tree
// with that module missing, each through a fresh public checker.
for (const item of plan.dotdot) record(item.name, await once(item.root, item.request));
for (const item of plan.dotdot) record(`directory:${item.name}`, await onceDirectory(item.root, item.request));

// S3c: project spellings through host links outside the directory root, in
// directory mode, and one in a mount, which has no host to ask.
for (const item of plan.g3.cases) record(item.name, await onceDirectory(item.root, item.request));
record("mount:g3-host-link", await once(plan.g3.mount.root, plan.g3.mount.request));
// The permission shapes: the denial checked real on this engine, then one
// check through the gate's observing worker, which reads back the store's
// own record at the denied path (an internal, read-only view; never the
// public API).
for (const item of plan.g3.permission) {
  const fs = await import("node:fs");
  const deniedBy = (action, code) => {
    try {
      action();
      return false;
    } catch (error) {
      return error?.code === code;
    }
  };
  const precondition = {
    denied: deniedBy(() => (item.denied.op === "lstat" ? fs.lstatSync(item.denied.path) : fs.readdirSync(item.denied.path)), "EACCES"),
    exists: item.exists === undefined ? undefined : fs.lstatSync(item.exists).isFile(),
  };
  const channel = new MessageChannel();
  const records = [];
  channel.port1.on("message", message => records.push(message));
  const worker = new Worker(new URL("./fault-worker.mjs", import.meta.url), {
    workerData: { mode: "observe", side: channel.port2, observe: item.observe },
    transferList: [channel.port2],
  });
  const exited = new Promise(resolve => worker.once("exit", resolve));
  const { checker, producer } = await loadArtifacts();
  const root = fs.realpathSync(item.root);
  const directory = { input: item.root, root, identity: identityOf(fs.lstatSync(root, { bigint: true })) };
  const api = await attach(worker, { checker, producer, directory, transfer: [checker, producer] });
  const result = await settle(api.check(item.request));
  await waitFor(() => records.some(message => message.type === "observation"), `${item.name} observation`);
  await api.close();
  await exited;
  channel.port1.close();
  record(item.name, { precondition, observation: records.find(message => message.type === "observation").outcome, ...result });
}

// S3d: inputs above the directory root, in directory mode, and two of them in
// a closed mount.
for (const item of plan.a2.cases) record(item.name, await onceDirectory(item.root, item.request));
for (const item of plan.a2.mount) record(item.name, await once(item.root, item.request));

// S3a: the control guests' real WASI path calls, through the owned runtime
// and, as the old-layer control, through the pinned upstream one.
{
  const semantics = await import("./path-semantics.mjs");
  const { WASI: Owned } = await import("solid-checker-wasm/vendor/wasm-util/lib/mjs/wasi/index.mjs");
  const { WASI: Old } = await import("@tybys/wasm-util");
  const { memfs } = await import("@napi-rs/wasm-runtime/fs");
  const { root, guests, mutations } = plan.pathSemantics;
  const rust = new WebAssembly.Module(readFileSync(guests.rust));
  const lookups = semantics.rows();
  const stdlib = semantics.stdlibRows(root);
  const layers = {};
  for (const [label, WASI] of [["owned", Owned], ["old", Old]]) {
    const run = (module, list, tree, checks) => semantics.runGuest({
      WASI, memfs, materializeMount, mount: validateMount(owned.mountDirectory(tree)), module, checks,
      rowArguments: list.map(row => semantics.guestArgument(row, tree)),
    });
    const runnable = list => (label === "old" ? list.filter(semantics.oldLayerRuns) : list);
    const results = run(rust, runnable([...lookups, ...stdlib.rust]), root);
    for (const { row, root: copy } of mutations) Object.assign(results, run(rust, [row], copy, { [row.id]: `${copy}/${row.check}` }));
    layers[label] = results;
  }
  record("path-semantics", layers);
}

// An unsupported installed runtime is answered before any producer program.
record("sc9013", await once(plan.cases.sc9013.root, plan.cases.sc9013.request));
record("directory:sc9013", await onceDirectory(plan.cases.sc9013.root, plan.cases.sc9013.request));

// Directory mode and a link leaving the root: refused only when a check
// reaches it, with the store's reason; an unreached one does not refuse.
{
  const root = plan.mounts.outsideLink;
  const unreached = await owned.createOwnedChecker({ directory: root });
  record("directory-outside-link-unreached", await settle(unreached.check({ projectId: `${root}/tsconfig.json` })));
  await unreached.close();
  const reached = await owned.createOwnedChecker({ directory: root });
  record("directory-outside-link-reached", await settle(reached.check({ projectId: `${root}/escape/tsconfig.json` })));
  await reached.close();
}

// What a worker may do under this run's permissions.
{
  const canary = await faultChecker("canary", owned.mountDirectory(w2.root), { outside: plan.outside });
  await waitFor(() => canary.records.some(item => item.type === "canary"), "the canary");
  await canary.finish();
  record("canary", canary.records.find(item => item.type === "canary"));
}

// W6: the Rust session's recorded update frames replayed into the packaged
// producer over the same tree.
{
  const directory = plan.updateFrames;
  const recording = JSON.parse(readFileSync(join(directory, "frames.json"), "utf8"));
  const { producer } = await loadArtifacts();
  const { WASI } = await import("solid-checker-wasm/vendor/wasm-util/lib/mjs/wasi/index.mjs");
  const { memfs } = await import("@napi-rs/wasm-runtime/fs");
  const { fs, vol } = memfs({}, "/");
  materializeMount(vol, validateMount(owned.mountDirectory(recording.root)));
  const wasi = new WASI({ version: "preview1", fs, preopens: { "/": "/" }, env: {} });
  const instance = new WebAssembly.Instance(await WebAssembly.compile(producer), wasi.getImportObject());
  wasi.initialize(instance);
  const exports = instance.exports;
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
  const replay = { root: recording.root, handshakeEqual: handshake.output.equals(readFileSync(join(directory, recording.handshake))), openStatus: opened.status, exchanges: [] };
  if (opened.status === 0) {
    const handle = opened.output.readUInt32LE(0);
    for (const exchange of recording.exchanges) {
      const answered = call("typefacts_lifecycle", readFileSync(join(directory, exchange.request)), handle);
      const expected = readFileSync(join(directory, exchange.response));
      replay.exchanges.push({
        operation: exchange.operation,
        status: answered.status,
        equal: answered.output.equals(expected),
        equalWithoutTimings: answered.status === 0 && withoutTimings(answered.output).equals(withoutTimings(expected)),
      });
    }
    replay.closeStatus = call("typefacts_close", undefined, handle).status;
  }
  record("w6-update-replay", replay);
}

writeFileSync(join(plan.out, "driver.json"), `${JSON.stringify(results, null, 2)}\n`);
