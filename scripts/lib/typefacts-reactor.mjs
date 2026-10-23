// Drives the Type Facts producer wasm reactor (apps/solid-typefacts's `reactor`
// feature) directly, under the package's WASI
// runtime with the host file system preopened at its own paths. This is a test and
// measurement driver. The shipped host is packages/wasm/owned-host.mjs, which
// serves the same exports through the owned boundary and a frozen mount.
import * as nodeFs from "node:fs";
import { readFileSync } from "node:fs";
// The package's pinned WASI runtime (packages/wasm/vendor/wasm-util/NOTICE):
// Bun's node:wasi lacks `initialize`, and this is the runtime the shipped host
// runs both reactors on, here over the real file system.
import { WASI } from "../../packages/wasm/vendor/wasm-util/lib/mjs/wasi/index.mjs";
import { boundaryImports } from "../../packages/wasm/owned-boundary.mjs";

const decoder = new TextDecoder();

/** One reactor instance. `wasm` is a path or the module's bytes. */
export async function loadReactor(wasm, { preopens = { "/": "/" }, env = {} } = {}) {
  const bytes = typeof wasm === "string" ? readFileSync(wasm) : wasm;
  const module = await WebAssembly.compile(bytes);
  const wasi = new WASI({ version: "preview1", args: [], env, preopens, fs: nodeFs });
  // The shipped host's boundary: WASI arguments arrive as signed i32, so a
  // reactor whose memory passes 2 GiB hands the raw runtime negative
  // addresses ("Offset is outside the bounds of the DataView"). The boundary
  // normalizes them to u32 and range-checks them, as owned-worker-core does.
  let memory;
  const strings = { args: [], env: Object.entries(env).map(([key, value]) => `${key}=${value}`) };
  const preview1 = boundaryImports(module, wasi.getImportObject().wasi_snapshot_preview1, () => memory, strings);
  const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: preview1 });
  memory = instance.exports.memory;
  wasi.initialize(instance);
  const exports = instance.exports;
  const call = (operation, input, ...rest) => {
    if (input) {
      const pointer = exports.allocate_input(input.length) >>> 0;
      new Uint8Array(exports.memory.buffer, pointer, input.length).set(input);
    }
    const status = exports[operation](...rest);
    const length = exports.output_length() >>> 0;
    const output = length === 0
      ? new Uint8Array(0)
      : new Uint8Array(exports.memory.buffer, exports.output_pointer() >>> 0, length).slice();
    return { status, output };
  };
  const succeed = (operation, result) => {
    if (result.status !== 0) throw new Error(`${operation}: ${decoder.decode(result.output)}`);
    return result.output;
  };
  return {
    exports,
    /** The raw status and output of `operation`, for tests of refusals. */
    call,
    handshake: () => Buffer.from(succeed("handshake", call("typefacts_handshake"))),
    open: envelope => new DataView(succeed("open", call("typefacts_open", envelope)).buffer).getUint32(0, true),
    lifecycle: (handle, frame) => Buffer.from(succeed("lifecycle", call("typefacts_lifecycle", frame, handle))),
    close: handle => void succeed("close", call("typefacts_close", undefined, handle)),
  };
}

// A deterministic CBOR encoder for the plain values a request holds (maps of
// text keys in length-first order, arrays, text, bytes, booleans, unsigned
// integers below 2^32), and its frame.
function head(major, value) {
  if (value < 24) return Buffer.from([(major << 5) | value]);
  if (value < 0x100) return Buffer.from([(major << 5) | 24, value]);
  if (value < 0x10000) {
    const bytes = Buffer.alloc(3);
    bytes[0] = (major << 5) | 25;
    bytes.writeUInt16BE(value, 1);
    return bytes;
  }
  const bytes = Buffer.alloc(5);
  bytes[0] = (major << 5) | 26;
  bytes.writeUInt32BE(value, 1);
  return bytes;
}

export function encodeCbor(value) {
  if (value === true) return Buffer.from([0xf5]);
  if (value === false) return Buffer.from([0xf4]);
  if (typeof value === "number") return head(0, value);
  if (typeof value === "string") {
    const text = Buffer.from(value);
    return Buffer.concat([head(3, text.length), text]);
  }
  if (Buffer.isBuffer(value)) return Buffer.concat([head(2, value.length), value]);
  if (Array.isArray(value)) return Buffer.concat([head(4, value.length), ...value.map(encodeCbor)]);
  const keys = Object.keys(value).sort((a, b) => a.length - b.length || (a < b ? -1 : 1));
  return Buffer.concat([head(5, keys.length), ...keys.flatMap(key => [encodeCbor(key), encodeCbor(value[key])])]);
}

export function requestFrame(value) {
  const payload = encodeCbor(value);
  const frame = Buffer.alloc(4 + payload.length);
  frame.writeUInt32LE(payload.length);
  payload.copy(frame, 4);
  return frame;
}

/** Decodes one CBOR item (unsigned and negative integers, bytes, text, arrays, maps, simple values). */
export function decodeCbor(bytes) {
  let at = 0;
  const argument = info => {
    if (info < 24) return info;
    const width = { 24: 1, 25: 2, 26: 4, 27: 8 }[info];
    if (!width) throw new Error(`unsupported CBOR additional information ${info}`);
    const value = bytes.readUIntBE(at + (width === 8 ? 4 : 0), width === 8 ? 4 : width);
    at += width;
    return value;
  };
  const item = () => {
    const initial = bytes[at++];
    const major = initial >> 5;
    const info = initial & 0x1f;
    if (major === 7) return { 20: false, 21: true, 22: null }[info];
    const count = argument(info);
    switch (major) {
      case 0: return count;
      case 1: return -1 - count;
      case 2: case 3: {
        const slice = bytes.subarray(at, at + count);
        at += count;
        return major === 3 ? slice.toString("utf8") : Buffer.from(slice);
      }
      case 4: return Array.from({ length: count }, item);
      case 5: {
        const map = {};
        for (let index = 0; index < count; index++) {
          const key = item();
          map[key] = item();
        }
        return map;
      }
      default: throw new Error(`unsupported CBOR major type ${major}`);
    }
  };
  const decoded = item();
  if (at !== bytes.length) throw new Error(`${bytes.length - at} trailing bytes after the CBOR item`);
  return decoded;
}

/** The payload of one length-prefixed frame, decoded. */
export function decodeFrame(frame) {
  const size = frame.readUInt32LE(0);
  if (size !== frame.length - 4) throw new Error(`frame prefix says ${size} bytes, holds ${frame.length - 4}`);
  return decodeCbor(frame.subarray(4));
}
