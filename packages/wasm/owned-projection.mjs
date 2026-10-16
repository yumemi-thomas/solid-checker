// The checker's `solid_owned_host` imports (P5 S3c, G3): one question about
// the request's project spelling, asked by `canonical_project`
// (solid-checker-wasm/src/projection.rs) only after WASI canonicalization
// failed with NotFound.
//
// The answer is computed by the directory store (`owned-snapshot.mjs`,
// `projectAnswer`) and encoded here against that store's own root, which is
// where an answer's paths are checked to lie inside or outside it: the
// checker's decoder checks only the answer's structure. A mount has no host
// to ask and always answers `unavailable`.
//
// Wire contract. Three imports, every parameter and result a wasm i32 read
// as u32, every range in the checker's memory, bound once after
// instantiation (a call before that, from the module's start or
// initialization included, throws `OwnedBoundaryError`):
//   project(path, length) -> status: computes and holds the answer for the
//     strict UTF-8 spelling at [path, path + length);
//   project_length() -> u32: the held answer's whole length, 0 if none;
//   project_copy(destination, length) -> status: copies the held answer,
//     `length` exactly, and releases it.
// Status 0 is ok, 1 a protocol refusal, 21 EFAULT (a range outside memory).
// A nonzero status installs, overwrites and releases nothing and touches no
// host or memory. A malformed argument throws. The answer is a host-owned
// copy, so memory growth between the calls cannot change it.
//
// Answer bytes: one tag, then its payload (integers little endian; paths
// strict UTF-8, absolute, clean, NUL-free):
//   0 unavailable, 1 not-outside, 6 faulted: nothing more;
//   2 projected: the path;
//   3 outside-root: u32 path length, the path, then the root;
//   4 name-variant: the first path whose last component is not an entry of
//     its parent's listing;
//   5 host-error: u16 WASI errno (ENOENT 44, ENOTDIR 54, EACCES 2, EPERM 63,
//     ELOOP 32);
//   7 listing-unavailable: u16 WASI errno (44, 54, 2, 63), then the directory
//     whose listing failed.
import { OwnedBoundaryError, u32 } from "./owned-boundary.mjs";

const EFAULT = 21;
const PROTOCOL = 1;
const MAX_ANSWER = 0xffffffff;

export const TAGS = Object.freeze({
  unavailable: 0, "not-outside": 1, projected: 2, "outside-root": 3,
  "name-variant": 4, "host-error": 5, faulted: 6, "listing-unavailable": 7,
});
export const HOST_ERRNOS = Object.freeze({ ENOENT: 44, ENOTDIR: 54, EACCES: 2, EPERM: 63, ELOOP: 32 });
const LISTING_ERRNOS = new Set([44, 54, 2, 63]);
const HOST_ERRNO_VALUES = new Set(Object.values(HOST_ERRNOS));

/** An internal invariant of an answer was violated: never sent to the checker. */
export class OwnedProjectionError extends Error {
  constructor(message) {
    super(message);
    this.name = "OwnedProjectionError";
  }
}

const textEncoder = new TextEncoder();
const strict = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });

/** `/`, or `/`-separated components none of which is empty, `.` or `..`. */
export function cleanAbsolute(path) {
  if (typeof path !== "string" || path.includes("\0") || !path.startsWith("/")) return false;
  if (path === "/") return true;
  return path.slice(1).split("/").every(component => component !== "" && component !== "." && component !== "..");
}

const within = (root, path) => path === root || path.startsWith(`${root}/`);

/**
 * The bytes of `answer` (`{ tag, ... }`), checked against the store `root`
 * the answer was computed for. Throws `OwnedProjectionError` on any answer
 * that store could not have given -- an injected one included.
 */
export function encodeProjection(answer, root) {
  const fail = why => {
    throw new OwnedProjectionError(`a project answer ${JSON.stringify(answer)} is not valid for root ${root}: ${why}`);
  };
  const tag = TAGS[answer?.tag];
  if (tag === undefined || typeof answer.tag !== "string" || !Object.hasOwn(TAGS, answer.tag)) fail("unknown tag");
  const path = (value, what) => {
    if (!cleanAbsolute(value)) fail(`${what} is not a clean absolute path`);
    const bytes = textEncoder.encode(value);
    // A lone surrogate would not survive the round trip as strict UTF-8.
    if (strict.decode(bytes) !== value) fail(`${what} is not representable as UTF-8`);
    return bytes;
  };
  const errno = (value, allowed) => {
    if (!allowed.has(value)) fail(`errno ${value} is not one this answer may carry`);
    return [value & 0xff, value >>> 8];
  };
  const keys = Object.keys(answer).filter(key => key !== "tag").sort().join(",");
  const only = expected => {
    if (keys !== expected) fail(`fields ${keys || "none"}, not ${expected || "none"}`);
  };
  if (root !== undefined && (!cleanAbsolute(root) || root === "/")) fail("the store root is not a clean absolute path other than /");
  const needsRoot = () => {
    if (root === undefined) fail("only a directory store answers this tag");
  };
  let payload;
  switch (answer.tag) {
    case "unavailable":
    case "not-outside":
    case "faulted":
      only("");
      payload = [];
      break;
    case "projected": {
      only("path");
      needsRoot();
      if (!within(root, answer.path)) fail("the projected path is not the root or below it");
      payload = [...path(answer.path, "the projected path")];
      break;
    }
    case "outside-root": {
      only("path,root");
      needsRoot();
      if (answer.root !== root) fail("the answer names another root");
      if (within(root, answer.path)) fail("the path is inside the root");
      const bytes = path(answer.path, "the path");
      const length = bytes.length;
      payload = [length & 0xff, (length >>> 8) & 0xff, (length >>> 16) & 0xff, length >>> 24, ...bytes, ...path(root, "the root")];
      break;
    }
    case "name-variant":
      only("path");
      needsRoot();
      if (answer.path === root || !within(root, answer.path)) fail("the name-variant path is not below the root");
      payload = [...path(answer.path, "the name-variant path")];
      break;
    case "host-error":
      only("errno");
      payload = errno(answer.errno, HOST_ERRNO_VALUES);
      break;
    case "listing-unavailable":
      only("directory,errno");
      needsRoot();
      if (!within(root, answer.directory)) fail("the listed directory is not the root or below it");
      payload = [...errno(answer.errno, LISTING_ERRNOS), ...path(answer.directory, "the listed directory")];
      break;
    default:
      fail("unknown tag");
  }
  if (payload.length + 1 > MAX_ANSWER) fail("the answer cannot be represented");
  return Uint8Array.from([tag, ...payload]);
}

/**
 * The three imports around `answerFor(spelling) -> Uint8Array`, with their
 * pending state. `bind(memory)` binds the checker's memory once;
 * `pending()` says whether an answer is held (the worker's completion
 * invariant); `clear()` releases it during terminal cleanup only.
 */
export function createProjectionHost(answerFor) {
  if (typeof answerFor !== "function") throw new OwnedBoundaryError("a projection host needs its answer function");
  let memory;
  let held = null;
  const bound = name => {
    if (!memory) throw new OwnedBoundaryError(`solid_owned_host.${name} called before the checker's memory is bound`);
    return memory.buffer;
  };
  const args = (name, raw, count) => {
    if (raw.length !== count) throw new OwnedBoundaryError(`${name} called with ${raw.length} arguments, not ${count}`);
    return raw.map((value, index) => u32(value, `${name} argument ${index}`));
  };
  const fits = (buffer, address, length) => address + length <= buffer.byteLength;
  const imports = Object.freeze({
    project: (...raw) => {
      const [address, length] = args("project", raw, 2);
      const buffer = bound("project");
      if (held !== null || length === 0) return PROTOCOL;
      if (!fits(buffer, address, length)) return EFAULT;
      let spelling;
      try {
        spelling = strict.decode(new Uint8Array(buffer, address, length));
      } catch {
        return PROTOCOL;
      }
      if (spelling.includes("\0")) return PROTOCOL;
      const answer = answerFor(spelling);
      if (!(answer instanceof Uint8Array) || answer.length === 0) {
        throw new OwnedBoundaryError("the projection answer is not a non-empty byte string");
      }
      // Discovered after the store was read: a terminal failure, not a
      // status, since the no-effect statuses cannot describe it. The worker
      // validates the store's records and poisons the checker.
      if (answer.length > MAX_ANSWER) throw new OwnedBoundaryError(`a projection answer of ${answer.length} bytes cannot be represented`);
      held = Uint8Array.from(answer);
      return 0;
    },
    project_length: (...raw) => {
      args("project_length", raw, 0);
      bound("project_length");
      return held === null ? 0 : held.length;
    },
    project_copy: (...raw) => {
      const [address, length] = args("project_copy", raw, 2);
      const buffer = bound("project_copy");
      if (held === null || length !== held.length) return PROTOCOL;
      if (!fits(buffer, address, length)) return EFAULT;
      new Uint8Array(buffer, address, length).set(held);
      held = null;
      return 0;
    },
  });
  return Object.freeze({
    imports,
    bind(checkerMemory) {
      if (memory) throw new OwnedBoundaryError("the checker's memory is already bound");
      if (!(checkerMemory instanceof WebAssembly.Memory)) throw new OwnedBoundaryError("the checker exports no memory");
      memory = checkerMemory;
    },
    pending: () => held !== null,
    clear() {
      held = null;
    },
  });
}

const TYPEFACTS_IMPORTS = new Set(["open", "handshake", "exchange", "result_length", "copy_result", "close"]);
const PROJECTION_IMPORTS = new Set(Object.keys(createProjectionHost(() => new Uint8Array(1)).imports));

/**
 * Refuses `module` unless every import it declares is a function this
 * worker serves to that reactor: preview1 names the boundary covers, and for
 * the checker alone the six `solid_typefacts` and three `solid_owned_host`
 * functions.
 */
export function checkImportInventory(module, role, coveredPreview1) {
  const allowed = {
    wasi_snapshot_preview1: new Set(coveredPreview1),
    ...(role === "checker" ? { solid_typefacts: TYPEFACTS_IMPORTS, solid_owned_host: PROJECTION_IMPORTS } : {}),
  };
  for (const { module: from, name, kind } of WebAssembly.Module.imports(module)) {
    if (kind !== "function" || !allowed[from]?.has(name)) {
      throw new OwnedBoundaryError(`the ${role} imports ${kind} ${from}.${name}, which the owned worker does not serve it`);
    }
  }
}
