// The owned boundary (P5 S3b, C): every preview1 import of the shipped
// reactors is covered by signature, arguments are normalized as the unsigned
// (or signed) values they encode, every addressed range is checked before
// the runtime is called, and malformed values are refused rather than
// coerced. Addresses above 2 GiB are exercised on one memory of 2 GiB + 64 KiB
// whose untouched pages are never backed.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import { WASI } from "@tybys/wasm-util";
import { memfs } from "@napi-rs/wasm-runtime/fs";
// The runtime the owned worker runs: the vendored copy, with the C-vendor
// repair (unsigned iovec buffer addresses).
import { WASI as OwnedWASI } from "../vendor/wasm-util/lib/mjs/wasi/index.mjs";
import { boundaryImports, coveredImports, OwnedBoundaryError, protocolImports, u32 } from "../owned-boundary.mjs";
import { createTransport } from "../owned-host.mjs";

const EFAULT = 21;
const HIGH = 0x80001000;
const asI32 = value => value | 0;
const memory = new WebAssembly.Memory({ initial: 32769 });
const bytes = () => new Uint8Array(memory.buffer);
const view = () => new DataView(memory.buffer);

// A module that only imports the named preview1 functions (type () -> ()).
function moduleImporting(names, from = "wasi_snapshot_preview1") {
  const text = value => [...new TextEncoder().encode(value)];
  const leb = value => {
    const out = [];
    do {
      let byte = value & 0x7f;
      value >>>= 7;
      if (value !== 0) byte |= 0x80;
      out.push(byte);
    } while (value !== 0);
    return out;
  };
  const entries = names.flatMap(name => [from.length, ...text(from), name.length, ...text(name), 0x00, 0x00]);
  const imports = [...leb(names.length), ...entries];
  return new WebAssembly.Module(new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
    0x02, ...leb(imports.length), ...imports,
  ]));
}

function spied(names, answer = () => 0) {
  const calls = [];
  const upstream = Object.fromEntries(names.map(name => [name, (...args) => {
    calls.push([name, args]);
    return answer(name, args);
  }]));
  return { calls, wrapped: boundaryImports(moduleImporting(names), upstream, () => memory, { args: ["a", "bc"], env: ["K=V"] }) };
}

test("every preview1 import of both shipped reactors is covered, and only those are wrapped", () => {
  const manifest = JSON.parse(readFileSync(new URL("../owned/manifest.json", import.meta.url), "utf8"));
  for (const role of ["checker", "producer"]) {
    const module = new WebAssembly.Module(readFileSync(new URL(`../owned/${manifest.artifacts[role].file}`, import.meta.url)));
    const names = WebAssembly.Module.imports(module).filter(entry => entry.module === "wasi_snapshot_preview1").map(entry => entry.name);
    assert.ok(names.length > 0);
    for (const name of names) assert.ok(coveredImports.includes(name), `${role} imports ${name}`);
    const upstream = new WASI({ version: "preview1" }).getImportObject().wasi_snapshot_preview1;
    assert.deepEqual(Object.keys(boundaryImports(module, upstream, () => memory, { args: [], env: [] })).sort(), [...new Set(names)].sort());
  }
});

test("an import the boundary does not know refuses the module", () => {
  assert.throws(() => boundaryImports(moduleImporting(["fd_advise"]), { fd_advise: () => 0 }, () => memory, { args: [], env: [] }),
    error => error instanceof OwnedBoundaryError && /fd_advise, which the owned boundary does not cover/.test(error.message));
  assert.throws(() => boundaryImports(moduleImporting(["fd_read"]), {}, () => memory, { args: [], env: [] }), OwnedBoundaryError);
});

test("addresses at and above 2 GiB reach the runtime unsigned, embedded iovecs included", () => {
  const { calls, wrapped } = spied(["fd_write", "path_open", "fd_seek", "clock_time_get"], () => 0x1234);
  view().setUint32(HIGH + 8, HIGH, true);
  view().setUint32(HIGH + 12, 5, true);
  assert.equal(wrapped.fd_write(1, asI32(HIGH + 8), 1, asI32(HIGH + 16)), 0x1234, "the runtime's answer is returned untouched");
  assert.deepEqual(calls.at(-1), ["fd_write", [1, HIGH + 8, 1, HIGH + 16]]);
  wrapped.path_open(3, 1, asI32(HIGH), 4, 0, -1n, 5n, 0, asI32(HIGH + 32));
  assert.deepEqual(calls.at(-1), ["path_open", [3, 1, HIGH, 4, 0, 2n ** 64n - 1n, 5n, 0, HIGH + 32]]);
  wrapped.fd_seek(3, -5n, 1, asI32(HIGH));
  assert.deepEqual(calls.at(-1), ["fd_seek", [3, -5n, 1, HIGH]], "the seek delta stays signed");
  wrapped.clock_time_get(0, -1n, asI32(HIGH));
  assert.deepEqual(calls.at(-1), ["clock_time_get", [0, 2n ** 64n - 1n, HIGH]]);
});

test("every addressed range must lie in memory, or the call answers EFAULT without the runtime", () => {
  const names = ["fd_read", "fd_write", "fd_pwrite", "path_filestat_get", "path_readlink", "fd_filestat_get", "fd_fdstat_get",
    "fd_prestat_get", "poll_oneoff", "args_get", "environ_get", "fd_readdir", "random_get", "fd_seek"];
  const { calls, wrapped } = spied(names);
  const end = memory.buffer.byteLength;
  const iovec = (at, buf, length) => {
    view().setUint32(at, buf, true);
    view().setUint32(at + 4, length, true);
    return at;
  };
  const refused = [
    () => wrapped.fd_read(3, asI32(end - 4), 1, 16),
    () => wrapped.fd_read(3, iovec(64, end - 2, 5), 1, 16),
    () => wrapped.fd_write(1, iovec(64, 0xfffffff0, 0x20), 1, 16),
    () => wrapped.fd_pwrite(3, iovec(64, 16, 4), 1, 0n, asI32(end - 3)),
    () => wrapped.path_filestat_get(3, 0, 16, 4, asI32(end - 63)),
    () => wrapped.path_readlink(3, 16, 4, asI32(end - 8), 9, 128),
    () => wrapped.fd_filestat_get(3, asI32(end - 63)),
    () => wrapped.fd_fdstat_get(3, asI32(end - 23)),
    () => wrapped.fd_prestat_get(3, asI32(end - 7)),
    () => wrapped.poll_oneoff(asI32(end - 48 * 2 + 1), 128, 2, 512),
    () => wrapped.poll_oneoff(128, asI32(end - 32 * 2 + 1), 2, 512),
    () => wrapped.args_get(asI32(end - 7), 128),
    () => wrapped.args_get(128, asI32(end - 4)),
    () => wrapped.environ_get(asI32(end - 3), 128),
    () => wrapped.environ_get(128, asI32(end - 3)),
    () => wrapped.fd_readdir(3, asI32(end - 10), 11, 0n, 128),
    () => wrapped.random_get(asI32(end - 1), 2),
    () => wrapped.fd_seek(3, 0n, 0, asI32(end - 7)),
  ];
  for (const [index, call] of refused.entries()) assert.equal(call(), EFAULT, `refused case ${index}`);
  assert.equal(calls.length, 0, "the runtime was never called");
  assert.equal(wrapped.random_get(asI32(end), 0), 0, "an empty range at the end of memory is in range");
  assert.equal(wrapped.args_get(asI32(end - 8), asI32(end - 5)), 0, "two pointers and five bytes of argv fit exactly");
  assert.equal(wrapped.fd_read(3, iovec(64, end - 5, 5), 1, 16), 0);
  assert.equal(calls.length, 3);
});

test("malformed values are refused, never coerced", () => {
  const { calls, wrapped } = spied(["fd_write", "fd_seek"]);
  for (const value of [Number.NaN, 1.5, 2 ** 32, -(2 ** 31) - 1, 1n, "1", undefined]) {
    assert.throws(() => wrapped.fd_write(value, 0, 0, 16), OwnedBoundaryError, String(value));
  }
  assert.throws(() => wrapped.fd_seek(3, 0, 0, 16), OwnedBoundaryError, "a Number in an i64 slot");
  assert.throws(() => wrapped.fd_write(1, 0, 0), OwnedBoundaryError, "a short call");
  assert.equal(calls.length, 0);
  assert.equal(u32(-1, "x"), 0xffffffff);
  assert.throws(() => u32(0x1_0000_0000, "x"), OwnedBoundaryError);
});

test("the checker's protocol imports are normalized as u32 and malformed values refused", () => {
  const seen = [];
  const imports = Object.fromEntries(["open", "handshake", "exchange", "result_length", "copy_result", "close"]
    .map(name => [name, (...args) => (seen.push([name, args]), 7)]));
  const wrapped = protocolImports(imports);
  assert.equal(wrapped.copy_result(asI32(0x80000005), asI32(HIGH), 16), 7);
  assert.deepEqual(seen.at(-1), ["copy_result", [0x80000005, HIGH, 16]]);
  assert.throws(() => wrapped.exchange(1, 0.5, 1), OwnedBoundaryError);
  assert.throws(() => wrapped.close(), OwnedBoundaryError);
});

test("the transport reads the producer's output above 2 GiB, and refuses an output outside its memory", () => {
  const producerMemory = memory;
  let outputAt = HIGH;
  bytes().set([1, 2, 3, 4], HIGH);
  const producer = {
    memory: producerMemory,
    allocate_input: () => asI32(HIGH + 4096),
    output_pointer: () => asI32(outputAt),
    output_length: () => 4,
    typefacts_handshake: () => 0,
    typefacts_open: () => 0,
    typefacts_lifecycle: () => 0,
    typefacts_close: () => 0,
  };
  const transport = createTransport(producer);
  const checkerMemory = new WebAssembly.Memory({ initial: 1 });
  transport.bind(checkerMemory);
  assert.equal(transport.imports.handshake(9), 0);
  assert.equal(transport.imports.result_length(9), 4);
  assert.equal(transport.imports.copy_result(9, 100, 4), 0);
  assert.deepEqual([...new Uint8Array(checkerMemory.buffer, 100, 4)], [1, 2, 3, 4]);
  outputAt = producerMemory.buffer.byteLength - 2;
  assert.throws(() => transport.imports.handshake(9), /producer output \[\d+, \d+\) is outside its memory/);
});

// The published runtime's synchronous iovec decoding reads the buffer address
// as signed (`getInt32`), so an address at or above 2 GiB selects memory
// relative to the end, silently. The vendored copy decodes it unsigned
// (C-vendor). Each sentinel distinguishes the two regions.
const sentinels = () => {
  bytes().set(new TextEncoder().encode("HIGH\n"), HIGH);
  // Where a signed `subarray(-0x7ffff000, …)` lands in this memory.
  bytes().set(new TextEncoder().encode("LOW!\n"), memory.buffer.byteLength - 0x7ffff000);
};

const writeSentinel = Runtime => {
  sentinels();
  const printed = [];
  const wasi = new Runtime({ version: "preview1", print: line => printed.push(line) });
  const module = moduleImporting(["fd_write"]);
  const wrapped = boundaryImports(module, wasi.getImportObject().wasi_snapshot_preview1, () => memory, { args: [], env: [] });
  wasi.initialize({ exports: { memory } });
  view().setUint32(HIGH + 64, HIGH, true);
  view().setUint32(HIGH + 68, 5, true);
  assert.equal(wrapped.fd_write(1, asI32(HIGH + 64), 1, asI32(HIGH + 72)), 0);
  return printed;
};

test("fd_write reads an iovec buffer above 2 GiB from that address through the owned runtime", () => {
  assert.deepEqual(writeSentinel(OwnedWASI), ["HIGH"]);
});

test("the published runtime reads the same iovec from the end-relative region (why C-vendor exists)", () => {
  assert.deepEqual(writeSentinel(WASI), ["LOW!"]);
});

test("fd_read fills an iovec buffer above 2 GiB at that address through the owned runtime", () => {
  const { fs, vol } = memfs({}, "/");
  vol.mkdirSync("/d", { recursive: true });
  vol.writeFileSync("/d/f.txt", "READ!");
  const wasi = new OwnedWASI({ version: "preview1", fs, preopens: { "/": "/" }, env: {} });
  const module = moduleImporting(["path_open", "fd_read"]);
  const wrapped = boundaryImports(module, wasi.getImportObject().wasi_snapshot_preview1, () => memory, { args: [], env: [] });
  wasi.initialize({ exports: { memory } });
  const path = new TextEncoder().encode("d/f.txt");
  bytes().set(path, 0x1000);
  assert.equal(wrapped.path_open(3, 1, 0x1000, path.length, 0, 2n, 0n, 0, 0x2000), 0);
  const fd = view().getUint32(0x2000, true);
  const target = HIGH + 256;
  const misread = memory.buffer.byteLength - (2 ** 32 - target);
  bytes().fill(0, target, target + 5);
  bytes().fill(0, misread, misread + 5);
  view().setUint32(HIGH + 128, target, true);
  view().setUint32(HIGH + 132, 5, true);
  assert.equal(wrapped.fd_read(fd, asI32(HIGH + 128), 1, asI32(HIGH + 136)), 0);
  assert.equal(view().getUint32(HIGH + 136, true), 5);
  assert.equal(new TextDecoder().decode(bytes().subarray(target, target + 5)), "READ!");
  assert.deepEqual([...bytes().subarray(misread, misread + 5)], [0, 0, 0, 0, 0], "nothing written at the end-relative region");
});
