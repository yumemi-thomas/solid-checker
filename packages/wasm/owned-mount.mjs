// The owned check's filesystem: one frozen, validated copy of a project tree
// that both reactors read through WASI (P5 S2).
//
// A mount is a root and its entries: regular files, directories and symbolic
// links at absolute normalized POSIX paths strictly under the root. Every
// entry is validated before anything is materialized, file bytes are copied
// into buffers this package owns, and links are created last, so no write
// ever passes through one. What the mount does not represent is outside the
// claim: nothing above the root exists in it (native discovery above the
// root has no counterpart here), and modes and permissions are not carried.
import { lstatSync, readFileSync, readdirSync, readlinkSync, realpathSync } from "node:fs";
import { posix, sep } from "node:path";

export class OwnedMountError extends Error {
  constructor(message) {
    super(message);
    this.name = "OwnedMountError";
  }
}

const textEncoder = new TextEncoder();

function isNormalAbsolute(path) {
  return typeof path === "string" && path.startsWith("/") && !path.includes("\0") &&
    posix.normalize(path) === path && (path === "/" || !path.endsWith("/"));
}

function inside(root, path) {
  return root === "/" ? path !== "/" : path.startsWith(`${root}/`);
}

const depth = path => path.split("/").length;

/**
 * Reads every property of a caller's mount exactly once, into inert records
 * this package owns: the root, the entries list and its length, and each
 * entry's kind, path and bytes or target. File bytes are copied as they are
 * read. Nothing after this reads the caller's objects again, so a getter or a
 * mutation cannot change what was validated.
 */
function capture(mount) {
  if (mount === null || typeof mount !== "object") {
    throw new OwnedMountError("mount must be an object with a root and an entries array");
  }
  const root = mount.root;
  const list = mount.entries;
  if (!Array.isArray(list)) {
    throw new OwnedMountError("mount must be an object with a root and an entries array");
  }
  const length = list.length;
  const entries = [];
  for (let index = 0; index < length; index++) {
    const entry = list[index];
    if (entry === null || typeof entry !== "object") throw new OwnedMountError("mount entry is not an object");
    const kind = entry.kind;
    const path = entry.path;
    if (kind === "file") {
      const bytes = entry.bytes;
      const owned = typeof bytes === "string" ? textEncoder.encode(bytes)
        : bytes instanceof Uint8Array ? new Uint8Array(bytes) : undefined;
      entries.push({ kind, path, bytes: owned });
    } else if (kind === "symlink") {
      entries.push({ kind, path, target: entry.target });
    } else {
      entries.push({ kind, path });
    }
  }
  return { root, entries };
}

/** Throws `OwnedMountError` unless every captured record is valid. */
function checkMount({ root, entries }) {
  if (!isNormalAbsolute(root)) {
    throw new OwnedMountError(`mount root ${JSON.stringify(root)} is not a normalized absolute POSIX path`);
  }
  const kinds = new Map();
  for (const { kind, path, bytes, target } of entries) {
    if (!isNormalAbsolute(path)) {
      throw new OwnedMountError(`mount entry ${JSON.stringify(path)} is not a normalized absolute POSIX path`);
    }
    if (!inside(root, path)) throw new OwnedMountError(`mount entry ${path} is not under the mount root ${root}`);
    if (kinds.has(path)) throw new OwnedMountError(`mount entry ${path} is declared twice`);
    if (kind === "file") {
      if (!(bytes instanceof Uint8Array)) {
        throw new OwnedMountError(`mount file ${path} has neither bytes nor text`);
      }
    } else if (kind === "symlink") {
      if (typeof target !== "string" || target === "" || target.includes("\0")) {
        throw new OwnedMountError(`mount link ${path} has no target`);
      }
      const resolved = target.startsWith("/")
        ? posix.normalize(target)
        : posix.normalize(posix.join(posix.dirname(path), target));
      if (resolved !== root && !inside(root, resolved)) {
        throw new OwnedMountError(`mount link ${path} -> ${target} leaves the mount root ${root}`);
      }
    } else if (kind !== "directory") {
      throw new OwnedMountError(`mount entry ${path} has unsupported kind ${JSON.stringify(kind)}`);
    }
    kinds.set(path, kind);
  }
  for (const path of kinds.keys()) {
    const parent = posix.dirname(path);
    if (parent !== root && kinds.get(parent) !== "directory") {
      throw new OwnedMountError(`mount entry ${path} has no declared parent directory ${parent}`);
    }
  }
}

/**
 * Validates a mount and returns this package's own frozen copy, entries in
 * materialization order (directories by depth, then files, then links).
 * The copy is exactly the records that were validated.
 */
export function validateMount(mount) {
  const captured = capture(mount);
  checkMount(captured);
  const { root } = captured;
  const directories = [];
  const files = [];
  const links = [];
  let bytes = 0;
  for (const entry of captured.entries) {
    if (entry.kind === "directory") directories.push(Object.freeze({ kind: "directory", path: entry.path }));
    else if (entry.kind === "symlink") links.push(Object.freeze({ kind: "symlink", path: entry.path, target: entry.target }));
    else {
      bytes += entry.bytes.byteLength;
      files.push(Object.freeze({ kind: "file", path: entry.path, bytes: entry.bytes }));
    }
  }
  directories.sort((a, b) => depth(a.path) - depth(b.path) || (a.path < b.path ? -1 : 1));
  return Object.freeze({ root, entries: Object.freeze([...directories, ...files, ...links]), bytes });
}

/** The file buffers a validated mount owns, for transfer to the worker. */
export function mountBuffers(mount) {
  return mount.entries.filter(entry => entry.kind === "file").map(entry => entry.bytes.buffer);
}

/** Writes a validated mount into an empty in-memory volume. */
export function materializeMount(vol, mount) {
  vol.mkdirSync(mount.root, { recursive: true });
  for (const entry of mount.entries) {
    if (entry.kind === "directory") vol.mkdirSync(entry.path);
    else if (entry.kind === "file") vol.writeFileSync(entry.path, entry.bytes);
    else vol.symlinkSync(entry.target, entry.path);
  }
}

/**
 * Snapshots a directory tree from the host filesystem as a mount: the root by
 * its real path, every entry by `lstat` (links are recorded, never followed).
 * Throws on an unreadable entry, an unsupported kind, or a link leaving the
 * root -- a mount that silently differed from the disk would not be the
 * project the native check reads. POSIX hosts only.
 */
export function mountDirectory(root) {
  if (sep !== "/") throw new OwnedMountError("mountDirectory supports POSIX hosts only");
  const real = realpathSync(root);
  if (!lstatSync(real).isDirectory()) throw new OwnedMountError(`mount root ${real} is not a directory`);
  const entries = [];
  const walk = directory => {
    for (const name of readdirSync(directory).sort()) {
      const path = posix.join(directory, name);
      const stat = lstatSync(path);
      if (stat.isSymbolicLink()) entries.push({ kind: "symlink", path, target: readlinkSync(path) });
      else if (stat.isDirectory()) {
        entries.push({ kind: "directory", path });
        walk(path);
      } else if (stat.isFile()) entries.push({ kind: "file", path, bytes: readFileSync(path) });
      else throw new OwnedMountError(`mount entry ${path} is not a regular file, directory or link`);
    }
  };
  walk(real);
  const mount = { root: real, entries };
  // These records are this function's own, built above, so they are checked
  // as they stand; `createOwnedChecker` captures them again like any input.
  checkMount(mount);
  return mount;
}
