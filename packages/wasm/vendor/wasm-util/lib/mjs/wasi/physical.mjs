// Physical path resolution for solid-checker's owned WASI worker.
//
// Owned by solid-checker; not part of @tybys/wasm-util. The pinned upstream
// preview1.mjs builds every path with path.mjs's lexical `resolve`, whose `..`
// step cuts at the first separator rather than the last, and which in any
// case cannot resolve `..` after a symbolic link the way the host does. The
// six path imports the owned reactors declare call this module instead; see
// ../../../NOTICE, ../../../UPSTREAM.json and ../../../owned.patch.
//
// Resolution walks the volume one component at a time, as POSIX path
// resolution does: a symbolic link met on the way is replaced by its target (a
// relative target from the link's own directory, an absolute one from `/`),
// `..` is the parent of the directory actually reached, every component before
// the last must be a directory (ENOTDIR) and must exist (ENOENT), and a
// trailing `/`, in the request or in any link target substituted into it, is
// a final `.`, so the name before it must resolve to a directory through
// links. The only root is `/`, the single preopen; no confinement to the
// directory fd itself is implied.
import { WasiErrno, WasiFileControlFlag } from "./types.mjs";
import { WasiError } from "./error.mjs";

/**
 * macOS MAXSYMLINKS, the measured native reference; not a universal bound.
 * Exported so the owned worker gives its directory store this same bound.
 */
export const MAX_SUBSTITUTIONS = 32;
const LOOKUP_SYMLINK_FOLLOW = 1;

// ENOENT, ENOTDIR and EEXIST travel as fs-style errors, which the upstream
// handleError maps exactly as it maps the memfs errors they replace.
function fsError(code, path) {
  const error = new Error(`${code}: ${path}`);
  error.code = code;
  return error;
}
const join = (directory, name) => (directory === "/" ? `/${name}` : `${directory}/${name}`);
function parentOf(directory) {
  const index = directory.lastIndexOf("/");
  return index <= 0 ? "/" : directory.slice(0, index);
}
function lstatOrMissing(fs, path) {
  try {
    return fs.lstatSync(path);
  } catch (error) {
    if (error.code === "ENOENT") return undefined;
    throw error;
  }
}

/**
 * The physical path `path` names from the directory `base` (an fd's
 * realPath). `follow` decides only whether a link in the final component is
 * followed. A missing final component yields the path it would have.
 */
export function resolvePhysical(fs, base, path, follow) {
  let current = path.startsWith("/") ? "/" : base;
  const pending = path.split("/");
  if (path.endsWith("/")) pending.push(".");
  const last = () => pending.every(name => name === "");
  let substitutions = 0;
  while (pending.length > 0) {
    const name = pending.shift();
    if (name === "") continue;
    const final = last();
    if (name === ".") {
      if (final) return current;
      continue;
    }
    if (name === "..") {
      current = parentOf(current);
      if (final) return current;
      continue;
    }
    const candidate = join(current, name);
    const stat = lstatOrMissing(fs, candidate);
    if (stat === undefined) {
      if (final) return candidate;
      throw fsError("ENOENT", candidate);
    }
    if (stat.isSymbolicLink() && (!final || follow)) {
      if (++substitutions > MAX_SUBSTITUTIONS) {
        throw new WasiError(`Too many levels of symbolic links: ${candidate}`, WasiErrno.ELOOP);
      }
      const target = String(fs.readlinkSync(candidate));
      if (target.startsWith("/")) current = "/";
      const components = target.split("/");
      if (target.endsWith("/")) components.push(".");
      pending.unshift(...components);
      continue;
    }
    if (final) return candidate;
    if (!stat.isDirectory()) throw fsError("ENOTDIR", candidate);
    current = candidate;
  }
  return current;
}

/**
 * The path path_open opens. The final link is followed only under the
 * lookup FOLLOW flag (O_DIRECTORY never follows); an unfollowed final link is
 * ELOOP as POSIX O_NOFOLLOW is, or ENOTDIR when O_DIRECTORY asked for a
 * directory (the macOS reference's answer); and O_CREAT|O_EXCL never follows
 * and fails EEXIST on any existing final entry, a link included.
 */
export function resolveOpenPath(fs, base, path, dirflags, oflags) {
  const exclusive = (oflags & WasiFileControlFlag.O_CREAT) !== 0 && (oflags & WasiFileControlFlag.O_EXCL) !== 0;
  const follow = (dirflags & LOOKUP_SYMLINK_FOLLOW) !== 0 && !exclusive;
  const resolved = resolvePhysical(fs, base, path, follow);
  if (exclusive) {
    if (lstatOrMissing(fs, resolved) !== undefined) throw fsError("EEXIST", resolved);
  } else if (!follow && lstatOrMissing(fs, resolved)?.isSymbolicLink()) {
    if ((oflags & WasiFileControlFlag.O_DIRECTORY) !== 0) throw fsError("ENOTDIR", resolved);
    throw new WasiError(`Symbolic link not followed: ${resolved}`, WasiErrno.ELOOP);
  }
  return resolved;
}

/**
 * The path a mutating import (mkdir, rmdir, unlink) acts on: its parent
 * resolved physically and its final name as given, with trailing slashes
 * dropped as the upstream lexical resolve dropped them. A final `.` or `..`
 * keeps the upstream meaning of the whole path, now resolved physically.
 */
export function resolveParentPath(fs, base, path) {
  const trimmed = path.replace(/\/+$/, "");
  const index = trimmed.lastIndexOf("/");
  const name = trimmed.slice(index + 1);
  if (name === "" || name === "." || name === "..") return resolvePhysical(fs, base, trimmed === "" ? path : trimmed, true);
  const directory = resolvePhysical(fs, base, index === -1 ? "." : trimmed.slice(0, index + 1), true);
  return join(directory, name);
}
