// The JS side of every call between a reactor and its imports (P5 S3b, C):
// a wasm32 address or length is an unsigned 32-bit value, but a WebAssembly
// `i32` reaches JavaScript as a signed number, so an address at or above
// 2 GiB arrives negative. Every preview1 import a reactor declares is wrapped
// by its signature: each argument is checked and normalized (`u32` with
// `>>> 0`, `u64` with `BigInt.asUintN`, the signed `filedelta` left as is),
// and every memory range the call will address -- path and output buffers,
// the iovec array and each iovec it lists, the poll arrays, the argv and
// environment tables and bytes -- must lie inside memory before the call is
// made, or it answers EFAULT, as a kernel does, without calling the runtime.
// A malformed argument (not an integer in the i32/u32 range, or not a BigInt
// for an i64) throws `OwnedBoundaryError`: nothing is coerced into a valid
// value and no range is clipped. Return values are not touched. An import
// this table does not know refuses the module.

export class OwnedBoundaryError extends Error {
  constructor(message) {
    super(message);
    this.name = "OwnedBoundaryError";
  }
}

const EFAULT = 21;
const textEncoder = new TextEncoder();

/** A wasm `i32` (or a value already in u32 range) as the u32 it encodes. */
export function u32(value, what) {
  if (typeof value !== "number" || !Number.isInteger(value) || value < -0x80000000 || value > 0xffffffff) {
    throw new OwnedBoundaryError(`${what} is not a 32-bit integer: ${String(value)}`);
  }
  return value >>> 0;
}

function i64(value, what) {
  if (typeof value !== "bigint") throw new OwnedBoundaryError(`${what} is not a 64-bit integer: ${String(value)}`);
  return value;
}

const normalize = {
  u32: (value, what) => u32(value, what),
  u64: (value, what) => BigInt.asUintN(64, i64(value, what)),
  s64: (value, what) => BigInt.asIntN(64, i64(value, what)),
};

/** The size `args_get`/`environ_get` writes for `strings` (wasm-util's own formula). */
const tableBytes = strings => textEncoder.encode(`${strings.join("\0")}\0`).length;

// Each row: the parameter kinds, then the ranges (address, length) the call
// addresses, from the normalized arguments, the memory's current view and
// the fixed WASI configuration. `iovecs(index, count)` adds the array and
// every element it lists.
const SIGNATURES = {
  args_get: [["u32", "u32"], (a, _, c) => [[a[0], 4 * c.args.length], [a[1], tableBytes(c.args)]]],
  args_sizes_get: [["u32", "u32"], a => [[a[0], 4], [a[1], 4]]],
  environ_get: [["u32", "u32"], (a, _, c) => [[a[0], 4 * c.env.length], [a[1], tableBytes(c.env)]]],
  environ_sizes_get: [["u32", "u32"], a => [[a[0], 4], [a[1], 4]]],
  clock_time_get: [["u32", "u64", "u32"], a => [[a[2], 8]]],
  random_get: [["u32", "u32"], a => [[a[0], a[1]]]],
  sched_yield: [[], () => []],
  proc_exit: [["u32"], () => []],
  poll_oneoff: [["u32", "u32", "u32", "u32"], a => [[a[0], 48 * a[2]], [a[1], 32 * a[2]], [a[3], 4]]],
  fd_close: [["u32"], () => []],
  fd_fdstat_get: [["u32", "u32"], a => [[a[1], 24]]],
  fd_fdstat_set_flags: [["u32", "u32"], () => []],
  fd_filestat_get: [["u32", "u32"], a => [[a[1], 64]]],
  fd_filestat_set_size: [["u32", "u64"], () => []],
  fd_prestat_get: [["u32", "u32"], a => [[a[1], 8]]],
  fd_prestat_dir_name: [["u32", "u32", "u32"], a => [[a[1], a[2]]]],
  fd_read: [["u32", "u32", "u32", "u32"], (a, iovecs) => [...iovecs(a[1], a[2]), [a[3], 4]]],
  fd_write: [["u32", "u32", "u32", "u32"], (a, iovecs) => [...iovecs(a[1], a[2]), [a[3], 4]]],
  fd_pread: [["u32", "u32", "u32", "u64", "u32"], (a, iovecs) => [...iovecs(a[1], a[2]), [a[4], 4]]],
  fd_pwrite: [["u32", "u32", "u32", "u64", "u32"], (a, iovecs) => [...iovecs(a[1], a[2]), [a[4], 4]]],
  fd_readdir: [["u32", "u32", "u32", "u64", "u32"], a => [[a[1], a[2]], [a[4], 4]]],
  fd_seek: [["u32", "s64", "u32", "u32"], a => [[a[3], 8]]],
  path_create_directory: [["u32", "u32", "u32"], a => [[a[1], a[2]]]],
  path_filestat_get: [["u32", "u32", "u32", "u32", "u32"], a => [[a[2], a[3]], [a[4], 64]]],
  path_open: [["u32", "u32", "u32", "u32", "u32", "u64", "u64", "u32", "u32"], a => [[a[2], a[3]], [a[8], 4]]],
  path_readlink: [["u32", "u32", "u32", "u32", "u32", "u32"], a => [[a[1], a[2]], [a[3], a[4]], [a[5], 4]]],
  path_remove_directory: [["u32", "u32", "u32"], a => [[a[1], a[2]]]],
  path_unlink_file: [["u32", "u32", "u32"], a => [[a[1], a[2]]]],
};

/** The preview1 names this boundary covers (for tests and reviews). */
export const coveredImports = Object.freeze(Object.keys(SIGNATURES).sort());

function inside(byteLength, address, length) {
  return address + length <= byteLength;
}

/**
 * The checked `wasi_snapshot_preview1` import object for `module`: exactly
 * the preview1 functions the module declares, each wrapped by its signature
 * around `upstream[name]`. `memory()` answers the instance's memory once it
 * exists; `config` is the WASI configuration's `args` and `env` strings.
 */
export function boundaryImports(module, upstream, memory, config) {
  const wrapped = {};
  for (const { module: from, name, kind } of WebAssembly.Module.imports(module)) {
    if (from !== "wasi_snapshot_preview1") continue;
    const signature = SIGNATURES[name];
    if (kind !== "function" || !signature || typeof upstream[name] !== "function") {
      throw new OwnedBoundaryError(`the reactor imports wasi_snapshot_preview1.${name}, which the owned boundary does not cover`);
    }
    const [kinds, ranges] = signature;
    const call = upstream[name];
    wrapped[name] = (...raw) => {
      if (raw.length !== kinds.length) {
        throw new OwnedBoundaryError(`${name} called with ${raw.length} arguments, not ${kinds.length}`);
      }
      const args = raw.map((value, index) => normalize[kinds[index]](value, `${name} argument ${index}`));
      const buffer = memory().buffer;
      const byteLength = buffer.byteLength;
      let fault = false;
      const iovecs = (address, count) => {
        const list = [[address, 8 * count]];
        if (!inside(byteLength, address, 8 * count)) {
          fault = true;
          return list;
        }
        const view = new DataView(buffer);
        for (let index = 0; index < count; index++) {
          list.push([view.getUint32(address + 8 * index, true), view.getUint32(address + 8 * index + 4, true)]);
        }
        return list;
      };
      const addressed = ranges(args, iovecs, config);
      if (fault || addressed.some(([address, length]) => !inside(byteLength, address, length))) return EFAULT;
      return call(...args);
    };
  }
  return wrapped;
}

/**
 * The checker's `solid_typefacts` imports with every argument normalized as
 * the u32 the Rust declaration names (`lib.rs`, `mod owned`). Ranges stay the
 * transport's own: it refuses an out-of-bounds pointer itself.
 */
export function protocolImports(imports) {
  const arity = { open: 3, handshake: 1, exchange: 3, result_length: 1, copy_result: 3, close: 1 };
  const wrapped = {};
  for (const [name, count] of Object.entries(arity)) {
    const call = imports[name];
    wrapped[name] = (...raw) => {
      if (raw.length !== count) throw new OwnedBoundaryError(`${name} called with ${raw.length} arguments, not ${count}`);
      return call(...raw.map((value, index) => u32(value, `${name} argument ${index}`)));
    };
  }
  return Object.freeze(wrapped);
}

const TYPEFACTS_IMPORTS = new Set(["open", "handshake", "exchange", "result_length", "copy_result", "close"]);

/**
 * Refuses `module` unless every import it declares is a function this
 * worker serves to that reactor: preview1 names the boundary covers, and for
 * the checker alone the six `solid_typefacts` functions.
 */
export function checkImportInventory(module, role, coveredPreview1 = coveredImports) {
  const allowed = {
    wasi_snapshot_preview1: new Set(coveredPreview1),
    ...(role === "checker" ? { solid_typefacts: TYPEFACTS_IMPORTS } : {}),
  };
  for (const { module: from, name, kind } of WebAssembly.Module.imports(module)) {
    if (kind !== "function" || !allowed[from]?.has(name)) {
      throw new OwnedBoundaryError(`the ${role} imports ${kind} ${from}.${name}, which the owned worker does not serve it`);
    }
  }
}
