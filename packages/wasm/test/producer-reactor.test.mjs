// The Type Facts producer reactor, driven directly under Node's WASI with the
// host file system preopened at its own paths (scripts/lib/typefacts-reactor.mjs).
// It runs against the producer `owned/` holds (TYPEFACTS_REACTOR_WASM names
// another module). The shipped path, through the owned boundary and a
// frozen mount, is covered by owned-worker-core.test.mjs and the parity gate.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, test } from "vitest";
import { decodeFrame, encodeCbor, loadReactor, requestFrame } from "../../../scripts/lib/typefacts-reactor.mjs";

const wasm = process.env.TYPEFACTS_REACTOR_WASM || new URL("../owned/solid-typefacts.wasm", import.meta.url).pathname;
const top = realpathSync(mkdtempSync(join(tmpdir(), "producer-reactor-")));
afterAll(() => rmSync(top, { recursive: true, force: true }));

const CONFIG = '{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}';
const decoder = new TextDecoder();

let reactor;
beforeAll(async () => {
  reactor = await loadReactor(wasm);
});

function project(name, files) {
  const root = join(top, name);
  mkdirSync(root, { recursive: true });
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  return root;
}

const requestOf = (config, id, operation, generation = 1, extra = {}) =>
  requestFrame({ schema: 1, operation, projectId: config, requestId: id, generation, ...extra });

test("the handshake is one length-prefixed frame carrying protocol, schema and build id", () => {
  const handshake = decodeFrame(reactor.handshake());
  assert.equal(typeof handshake.protocol, "number");
  assert.match(handshake.schemaHash, /^sha256:[0-9a-f]{64}$/);
  assert.equal(typeof handshake.buildId, "string");
});

test("a refusal answers status 1 with its message and issues no handle", () => {
  const refused = bytes => {
    const { status, output } = reactor.call("typefacts_open", bytes);
    assert.equal(status, 1);
    return decoder.decode(output);
  };
  assert.equal(refused(encodeCbor({ projectId: "tsconfig.json" })), 'project identity "tsconfig.json" is not a clean absolute path');
  assert.equal(refused(encodeCbor({ projectId: "/a/../tsconfig.json" })), 'project identity "/a/../tsconfig.json" is not a clean absolute path');
  assert.equal(refused(encodeCbor({ config: Buffer.from("{}"), projectId: "/p/tsconfig.json" })),
    "open request carries configuration bytes without hasConfig");
  const missing = join(top, "missing", "tsconfig.json");
  assert.equal(refused(encodeCbor({ projectId: missing })), `open TypeScript project: parse tsconfig: TS5083: Cannot read file '${missing}'.`);
});

test("handles are issued in order, never reused, and refused when foreign or closed", () => {
  const root = project("handles", { "tsconfig.json": CONFIG, "a.ts": "export const a = 1;\n" });
  const config = join(root, "tsconfig.json");
  const first = reactor.open(encodeCbor({ projectId: config }));
  const second = reactor.open(encodeCbor({ projectId: config }));
  assert.equal(second, first + 1);
  const opened = decodeFrame(reactor.lifecycle(first, requestOf(config, 1, "open")));
  assert.equal(opened.ok, true);
  reactor.close(first);
  reactor.close(first);
  const closed = reactor.call("typefacts_lifecycle", requestOf(config, 2, "open"), first);
  assert.equal(closed.status, 1);
  assert.equal(decoder.decode(closed.output), "handle is closed");
  const foreign = reactor.call("typefacts_close", undefined, second + 1000);
  assert.equal(foreign.status, 1);
  assert.equal(decoder.decode(foreign.output), "handle was never issued by this reactor");
  assert.equal(reactor.open(encodeCbor({ projectId: config })), second + 1);
  reactor.close(second);
  reactor.close(second + 1);
});

test("an overlay configuration stands in for a file that is not on disk", () => {
  const root = project("overlay", { "a.ts": "export const a = 1;\n" });
  const logical = join(root, "tsconfig.generated.json");
  const handle = reactor.open(encodeCbor({ projectId: logical, hasConfig: true, config: Buffer.from(CONFIG) }));
  const sources = decodeFrame(reactor.lifecycle(handle, requestOf(logical, 1, "sources")));
  assert.equal(sources.ok, true);
  assert.deepEqual(sources.sources.map(source => source.path), [join(root, "a.ts")]);
  assert.equal(decoder.decode(sources.sources[0].source), "export const a = 1;\n");
  reactor.close(handle);
});

function runtimeIdentities(config, file, text) {
  const handle = reactor.open(encodeCbor({ projectId: config }));
  reactor.lifecycle(handle, requestOf(config, 1, "open"));
  const start = text.indexOf("value");
  const answer = reactor.lifecycle(handle, requestOf(config, 2, "analyze", 1, {
    resetState: true,
    demands: [{ location: { path: file, startByte: start, endByte: start + "value".length }, symbol: true, runtimeIdentity: true }],
  }));
  reactor.close(handle);
  // The identities sit in the packed table as plain text.
  return [...answer.toString("latin1").matchAll(/runtime:h:[0-9a-f]+/g)].map(match => match[0]);
}

test("a project opened through a symlinked root names the same runtime symbol as the real root", () => {
  const text = "export const value = 1;\n";
  const real = project("real-root", { "tsconfig.json": CONFIG, "a.ts": text });
  const link = join(top, "linked-root");
  symlinkSync(real, link);
  const viaReal = runtimeIdentities(join(real, "tsconfig.json"), join(real, "a.ts"), text);
  const viaLink = runtimeIdentities(join(link, "tsconfig.json"), join(link, "a.ts"), text);
  assert.equal(viaReal.length, 1);
  // The identity hashes the symlink-resolved path: it does not see the link.
  assert.deepEqual(viaLink, viaReal);
});

test("a symlinked node_modules package resolves to its real file and reports both paths", () => {
  const root = project("symlinked-package", {
    "tsconfig.json": CONFIG,
    "main.ts": 'import { x } from "pkg";\nexport const y = x;\n',
    "libs/pkg/package.json": '{"name":"pkg","version":"1.0.0","types":"index.d.ts"}',
    "libs/pkg/index.d.ts": "export declare const x: number;\n",
  });
  mkdirSync(join(root, "node_modules"));
  symlinkSync("../libs/pkg", join(root, "node_modules/pkg"));
  const config = join(root, "tsconfig.json");
  const handle = reactor.open(encodeCbor({ projectId: config }));
  reactor.lifecycle(handle, requestOf(config, 1, "open"));
  const answer = decodeFrame(reactor.lifecycle(handle, requestOf(config, 2, "modules", 1, {
    moduleGraph: { imports: true, importPaths: [join(root, "main.ts")], packages: true },
  })));
  reactor.close(handle);
  assert.equal(answer.ok, true);
  const real = join(root, "libs/pkg/index.d.ts");
  assert.ok(answer.modules.some(module => module.path === real), "the module is the real file");
  const [imported] = answer.moduleImports;
  assert.equal(answer.moduleImports.length, 1);
  assert.equal(imported.text, "pkg");
  assert.equal(imported.resolution, "nodeModules");
  assert.equal(imported.resolvedPath, real);
  assert.equal(imported.symlinkPath, join(root, "node_modules/pkg/index.d.ts"));
  assert.equal(imported.resolverPackage.name, "pkg");
});

// A chain of incremental updates with a full rebuild every tenth: the work
// that frees program generations. Returns the reactor's linear memory in MiB.
function churn(instance, name, updates) {
  const root = project(name, { "tsconfig.json": CONFIG, "a.ts": "export const value = 0;\n", "b.ts": "import { value } from './a';\nexport const w = value + 1;\n" });
  const config = join(root, "tsconfig.json");
  const handle = instance.open(encodeCbor({ projectId: config }));
  instance.lifecycle(handle, requestOf(config, 1, "open"));
  for (let index = 1; index <= updates; index++) {
    const changes = [{ path: join(root, "a.ts"), source: Buffer.from(`export const value = ${index};\n`), version: index, deleted: false }];
    // A configuration edit forces a full rebuild; the others are incremental.
    if (index % 10 === 0) {
      const edited = CONFIG.replace('"target":"esnext"', `"target":"esnext","noEmit":${index % 20 === 0}`);
      changes.push({ path: config, source: Buffer.from(edited), version: index, deleted: false });
    }
    const answer = decodeFrame(instance.lifecycle(handle, requestOf(config, index + 1, "update", index + 1, { changes })));
    assert.equal(answer.ok, true, `update ${index}`);
  }
  instance.lifecycle(handle, requestOf(config, updates + 2, "close", updates + 1));
  instance.close(handle);
  return instance.exports.memory.buffer.byteLength / 2 ** 20;
}

test("freeing generations on wasm: updates and rebuilds keep linear memory bounded", () => {
  const updates = 300;
  // Observed about 156 MiB for the reactor (80 MiB of it the shadow
  // stack); without freeing the Rust one reaches gigabytes.
  const mebibytes = churn(reactor, "memory", updates);
  assert.ok(mebibytes < 200, `linear memory is ${mebibytes} MiB after ${updates} updates`);
}, 120_000);

// Linear memory only grows, and a healthy reactor still grows in occasional
// one-time steps (a larger peak allocation, a rebuild needing a bigger arena):
// 12 MiB at one update and nothing for the next 200, measured on this churn. A
// leak grows in every window. So the verdict is the smallest growth over equal
// consecutive windows: a step lands in one window and the others stay flat,
// while a per-update leak shows in all of them.
const LEAK_WINDOW_UPDATES = 500;
const LEAK_WINDOWS = 3;
const FLAT_WINDOW_MEBIBYTES = 8;
function leakedMebibytes(readings) {
  const growth = readings.slice(1).map((reading, index) => reading - readings[index]);
  return Math.min(...growth);
}

test("a leak per update grows every window; a one-time step grows only one", () => {
  // The regression this guards grew about 0.26 MiB per update.
  const leaking = Array.from({ length: LEAK_WINDOWS + 1 }, (_, window) => 150 + window * LEAK_WINDOW_UPDATES * 0.26);
  assert.ok(leakedMebibytes(leaking) > 10 * FLAT_WINDOW_MEBIBYTES);
  // Even a tenth of it fails.
  const slow = Array.from({ length: LEAK_WINDOWS + 1 }, (_, window) => 150 + window * LEAK_WINDOW_UPDATES * 0.026);
  assert.ok(leakedMebibytes(slow) > FLAT_WINDOW_MEBIBYTES);
  const stepped = [158.1, 158.1, 174.9, 174.9];
  assert.ok(leakedMebibytes(stepped) < FLAT_WINDOW_MEBIBYTES);
});

test("the high-water mark is flat: later windows of updates add no memory", () => {
  // Before the Rust reactor freed edited file versions and parsed
  // configurations it grew about 0.26 MiB per update on this churn.
  const instance = reactor;
  churn(instance, "flat-warmup", 300);
  const readings = [instance.exports.memory.buffer.byteLength / 2 ** 20];
  for (let window = 0; window < LEAK_WINDOWS; window++) {
    readings.push(churn(instance, `flat-window-${window}`, LEAK_WINDOW_UPDATES));
  }
  assert.ok(
    leakedMebibytes(readings) < FLAT_WINDOW_MEBIBYTES,
    `linear memory grew in every window of ${LEAK_WINDOW_UPDATES} updates: ${readings.map(reading => reading.toFixed(1)).join(" -> ")} MiB`,
  );
}, 300_000);

test("freed generations are never read: the same churn with TSRS_ARENA_POISON=1", async () => {
  // The Rust reactor then fills freed arena memory and never reuses it, so a
  // read of freed memory returns poison instead of a stale answer.
  const poisoned = await loadReactor(wasm, { env: { TSRS_ARENA_POISON: "1" } });
  churn(poisoned, "poisoned", 60);
}, 120_000);
