// The owned check's directory store (P5 S3b, B; S3d): a project read from
// the host disk on demand, recorded once, and shared by both reactors as
// their only filesystem.
//
// What it answers. Every guest-visible answer is computed from host records
// keyed by (primitive, physical path), each captured at most once:
//   L  lstat: the stats and identity (dev, ino, mode, nlink, size, mtimeNs,
//      ctimeNs), or the errno;
//   R  readlink: the target bytes, or the errno (EINVAL for a non-link);
//   D  readdir, bracketed by two lstats: the ordered names and types, or
//      the errno;
//   F  open: for a file its bytes (fstat before and after equal to L), for
//      a directory the open succeeding, or the open errno.
// Paths are resolved as a kernel resolves one syscall (cwd "/"): component
// by component through L records, `..` against the directory reached,
// links substituted at most `links` times (the WASI runtime's own bound),
// the final link followed only where the syscall follows it.
//
// The namespace is the host's (S3d). Native discovery reads above the
// project without bound -- the nearest `node_modules/solid-js` for the
// dialect, TypeScript's `node_modules` and `package.json` lookups, a
// workspace root for host inference, `extends` and `paths` above the
// project -- so a path anywhere, `/` and the root's ancestors included, is
// answered from its own host records, and a link is followed wherever its
// target leads, as the kernel follows it. Nothing outside the directory is
// answered absent unless the host says so. The root is the canonical
// directory the checker was created for: its identity is checked at start
// and on every validation, and it anchors the precondition below.
//
// What it claims. The caller keeps the host namespace from `/` through the
// root, and everything a check reaches -- inside the root or not --
// unchanged while the checker lives (premise P). The store issues Node
// path-string syscalls, so it is no
// sandbox: it cannot stop the OS following a link someone swaps in. After
// each check, `validate()` re-performs every record ever captured and
// compares it exactly: L by identity, R by target bytes, D by listing (its
// entries, or its errno), F by L identity or by the open errno. The identity
// is (dev, ino, mode, nlink, size, mtimeNs, ctimeNs) for every file and link
// and for every directory inside the root; for a directory outside the root
// it is (dev, ino, mode) only, because such a directory is shared with the
// rest of the host and its timestamps, size and link count move with
// unrelated activity beside the paths a check walks through it. Its entries
// that a check reaches are their own records, and a listing of it is its own
// record, so a replaced directory or a changed reached entry or listing is
// detected; a change to only its timestamps, size or link count is not, and
// those fields are answered from the record, not claimed current (no
// consumer in the reactors is known to read them; that is a source
// observation, not a guarantee). If every record validates, and a change
// always changes the identity (no same-size rewrite restored within one
// timestamp tick: coarse-clock filesystems weaken this), there was an
// instant between the last capture and the start of validation at which the
// host held every recorded file's bytes, identity, link target, listing and
// error. Stat fields outside the identity (atime, which reading itself may
// change; blksize; blocks) are captured once and are not part of that claim.
//
// The preopen. The WASI runtime opens `/` once when it is constructed, as its
// one preopen: an anchor for resolving every path, which it never reads (a
// listing is taken by path, a directory descriptor is only ever fstat'd).
// Native analysis needs only search permission on `/` to reach what lies
// below it. So that one open, inside `preopen(construct)`, is answered from
// `/`'s L record without a host open; every other open or listing of `/` --
// a guest's own -- is a real host operation, recorded and denied as the host
// denies it.
//
// What it refuses. Any other host error, a capture that changed while it was
// taken, a kind other than file, directory or link, a name or target that is
// not UTF-8, any write, and any call outside the consumed surface raise an
// `OwnedInputFault`. The first fault is sticky: it is thrown again by every
// later call, before any host access, and the worker discards the check.
// An error is never answered as absence or as empty content.
import * as nodeFs from "node:fs";
import { posix } from "node:path";

export class OwnedInputFault extends Error {
  constructor(message) {
    super(message);
    this.name = "OwnedInputFault";
  }
}

// wasm-util's `FileControlFlag` values (Linux numbering), not the host's.
// The access mode (O_WRONLY 1, O_RDWR 2) is not a write: wasm-util derives it
// from the rights a guest requests, and the Go producer's read-only opens
// carry FD_FILESTAT_SET_SIZE, so they arrive as O_RDWR. The host file is only
// ever opened O_RDONLY, and every write through the descriptor faults. The
// flags that change the file at open are refused before any host access.
const O_ACCMODE = 3;
const MODIFYING_FLAGS = 64 | 512 | 1024 | 1052672; // O_CREAT, O_TRUNC, O_APPEND, O_SYNC
const O_EXCL = 128;
const O_NONBLOCK = 2048;
const O_DIRECTORY = 65536;
const O_NOFOLLOW = 131072;
const FIRST_FD = 2 ** 30;

const EXPECTED = {
  L: new Set(["ENOENT", "ENOTDIR", "EACCES", "EPERM"]),
  R: new Set(["EINVAL", "ENOENT", "ENOTDIR", "EACCES", "EPERM"]),
  D: new Set(["ENOENT", "ENOTDIR", "EACCES", "EPERM"]),
  F: new Set(["ENOENT", "ENOTDIR", "EACCES", "EPERM"]),
};

function direntType(dirent) {
  if (dirent.isFile()) return "file";
  if (dirent.isDirectory()) return "directory";
  if (dirent.isSymbolicLink()) return "symlink";
  if (dirent.isFIFO()) return "fifo";
  if (dirent.isSocket()) return "socket";
  if (dirent.isCharacterDevice()) return "character";
  if (dirent.isBlockDevice()) return "block";
  return "unknown";
}

/**
 * A directory's entries as `{ name: Uint8Array, type }`, sorted by name
 * bytes: Node's libuv sorts a listing so and Bun returns the OS order, and
 * the guest must see one order on both. The raw name bytes and the Dirent
 * types come from two listings (Bun ignores `withFileTypes` with
 * `encoding: "buffer"`); they must name the same entries in the same order.
 */
function listDirectory(path) {
  const raw = nodeFs.readdirSync(path, { encoding: "buffer" });
  const typed = nodeFs.readdirSync(path, { withFileTypes: true });
  // `ignoreBOM`: a leading U+FEFF is part of a name, never an encoding mark.
  const lossy = new TextDecoder("utf-8", { ignoreBOM: true });
  if (raw.length !== typed.length || raw.some((name, index) => lossy.decode(name) !== typed[index].name)) {
    throw new Error("the listing changed between its two reads");
  }
  return raw.map((name, index) => ({ name: new Uint8Array(name), type: direntType(typed[index]) }))
    .sort((a, b) => Buffer.compare(a.name, b.name));
}

/** The host, read through Node's synchronous path-string calls. */
export const hostSource = Object.freeze({
  realpath: path => nodeFs.realpathSync(path),
  lstat: path => nodeFs.lstatSync(path, { bigint: true }),
  readlink: path => nodeFs.readlinkSync(path, { encoding: "buffer" }),
  readdir: listDirectory,
  open: path => nodeFs.openSync(path, nodeFs.constants.O_RDONLY | nodeFs.constants.O_NOFOLLOW),
  fstat: fd => nodeFs.fstatSync(fd, { bigint: true }),
  read: (fd, buffer, offset, length, position) => nodeFs.readSync(fd, buffer, offset, length, position),
  close: fd => nodeFs.closeSync(fd),
});

/** The identity a change must alter (A1): everything but atime and blocks. */
export const identityOf = stats =>
  [stats.dev, stats.ino, stats.mode, stats.nlink, stats.size, stats.mtimeNs, stats.ctimeNs].join(",");

// Names and link targets are decoded strictly and whole: invalid UTF-8 is a
// fault, and a leading U+FEFF (EF BB BF) is kept as the character it is.
const utf8 = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });

function kindOf(stats) {
  if (stats.isFile()) return "file";
  if (stats.isDirectory()) return "directory";
  if (stats.isSymbolicLink()) return "symlink";
  return undefined;
}

function dirent(name, type) {
  return Object.freeze({
    name,
    isFile: () => type === "file",
    isDirectory: () => type === "directory",
    isSymbolicLink: () => type === "symlink",
    isFIFO: () => type === "fifo",
    isSocket: () => type === "socket",
    isCharacterDevice: () => type === "character",
    isBlockDevice: () => type === "block",
  });
}

// The errors this store answers, as memfs throws them: an `Error` whose
// `code` wasm-util's `handleError` maps. Only these leave a call unfaulted.
const answers = new WeakSet();
const expected = code => {
  const error = Object.assign(new Error(code), { code });
  answers.add(error);
  return error;
};

/**
 * A store for the canonical directory `root`, whose identity on the main
 * thread was `rootIdentity`, reached from the caller's `input` spelling.
 *
 * `loopError(message)` builds the ELOOP error the WASI runtime maps (its own
 * `WasiError` module instance), and `links` is that runtime's substitution
 * bound (its physical resolver's `MAX_SUBSTITUTIONS`), so one bound governs a
 * WASI call and a host syscall alike; the owned worker passes both. `source`
 * is the host (the default); tests replace it. None is reachable from
 * `createOwnedChecker`.
 */
export function createDirectoryStore({ input, root, rootIdentity, loopError, links, source = hostSource }) {
  if (typeof root !== "string" || !root.startsWith("/") || root === "/" || posix.normalize(root) !== root || root.endsWith("/")) {
    throw new OwnedInputFault(`directory root ${JSON.stringify(root)} is not a canonical absolute path`);
  }
  if (typeof loopError !== "function" || !Number.isSafeInteger(links) || links < 1) {
    throw new OwnedInputFault("a directory store needs the WASI runtime's loop error and link bound");
  }
  const records = new Map();
  const order = [];
  const fds = new Map();
  const loops = new WeakSet();
  let nextFd = FIRST_FD;
  let fault;
  // While `preopen` constructs a WASI runtime: `constructing` for the whole
  // of `construct`, `anchorAvailable` until its one open of `/` is answered.
  let constructing = false;
  let anchorAvailable = false;

  const raise = (path, reason) => {
    fault ??= new OwnedInputFault(`the owned checker's input faulted: ${path}: ${reason}`);
    return fault;
  };
  const guard = () => {
    if (fault) throw fault;
  };
  const describe = error => error?.code ?? `${error?.name ?? "Error"}: ${error?.message ?? error}`;

  // A directory outside the root is shared with the rest of the host: its
  // mtime, ctime, size and link count move whenever anything is created
  // beside the paths a check walks through it ($TMPDIR, a home directory).
  // What resolving through it consumes is that it is the same directory
  // (dev, ino, mode); each entry a check reaches is its own record, and a
  // listing it takes is its own record. Inside the root, which the caller
  // vouches for, a directory's full identity stands.
  const inside = path => path === root || path.startsWith(`${root}/`);
  const identityAt = (path, stats, directory = stats.isDirectory()) => (directory && !inside(path)
    ? ["directory", stats.dev, stats.ino, stats.mode].join(",")
    : identityOf(stats));

  // One host call, its expected errors as an outcome and anything else a fault.
  const attempt = (primitive, path, action) => {
    try {
      return { ok: true, value: action() };
    } catch (error) {
      if (error instanceof OwnedInputFault) throw error;
      if (EXPECTED[primitive].has(error?.code)) return { ok: false, code: error.code };
      throw raise(path, `${primitive} failed: ${describe(error)}`);
    }
  };
  const store = (key, record) => {
    if (records.has(key)) throw raise(key, "a record would be captured twice");
    records.set(key, record);
    order.push(key);
    return record;
  };

  const captureL = path => {
    const outcome = attempt("L", path, () => source.lstat(path));
    if (!outcome.ok) return { ok: false, code: outcome.code };
    const stats = outcome.value;
    if (!kindOf(stats)) throw raise(path, "is not a regular file, directory or link");
    return { ok: true, stats: Object.freeze(stats), identity: identityAt(path, stats) };
  };
  const captureR = path => {
    const outcome = attempt("R", path, () => source.readlink(path));
    if (!outcome.ok) return { ok: false, code: outcome.code };
    const bytes = new Uint8Array(outcome.value);
    let text;
    try {
      text = utf8.decode(bytes);
    } catch {
      throw raise(path, "link target is not UTF-8");
    }
    return { ok: true, bytes, text };
  };
  const identityNow = path => {
    try {
      return identityAt(path, source.lstat(path));
    } catch (error) {
      throw raise(path, `changed while it was captured (${describe(error)})`);
    }
  };
  const captureD = path => {
    const before = identityNow(path);
    const outcome = attempt("D", path, () => source.readdir(path));
    const after = identityNow(path);
    if (before !== after) throw raise(path, "changed while it was listed");
    if (!outcome.ok) return { ok: false, code: outcome.code, identity: before };
    const entries = outcome.value.map(entry => {
      let name;
      try {
        name = utf8.decode(entry.name);
      } catch {
        throw raise(path, "lists a name that is not UTF-8");
      }
      return [name, entry.type];
    });
    return { ok: true, entries, identity: before };
  };
  const captureF = (path, lstat) => {
    const opened = attempt("F", path, () => source.open(path));
    if (!opened.ok) return { ok: false, code: opened.code };
    const fd = opened.value;
    let bytes = null;
    try {
      const sameAs = (stats, when) => {
        if (identityAt(path, stats, lstat.stats.isDirectory()) !== lstat.identity) throw raise(path, `changed ${when} it was read`);
      };
      const first = source.fstat(fd);
      sameAs(first, "before");
      if (first.isFile()) {
        const size = Number(first.size);
        bytes = new Uint8Array(size);
        let offset = 0;
        while (offset < size) {
          const read = source.read(fd, bytes, offset, size - offset, offset);
          if (read === 0) throw raise(path, "changed while it was read (short read)");
          offset += read;
        }
        if (source.read(fd, new Uint8Array(1), 0, 1, size) !== 0) throw raise(path, "changed while it was read (grew)");
      }
      sameAs(source.fstat(fd), "while");
    } catch (error) {
      if (error instanceof OwnedInputFault) throw error;
      throw raise(path, `F failed: ${describe(error)}`);
    } finally {
      try {
        source.close(fd);
      } catch (error) {
        // A close failure after a fault keeps the first fault.
        raise(path, `close failed: ${describe(error)}`);
      }
    }
    guard();
    return { ok: true, identity: lstat.identity, bytes };
  };

  const record = (primitive, path, capture) => {
    const key = `${primitive}\0${path}`;
    return records.get(key) ?? store(key, capture());
  };
  const L = path => record("L", path, () => captureL(path));
  const R = path => record("R", path, () => captureR(path));
  const D = path => record("D", path, () => captureD(path));
  const F = (path, lstat) => record("F", path, () => captureF(path, lstat));

  // The init chain: the caller's spelling still names this root, by identity.
  const captureRoot = () => {
    let real;
    try {
      real = source.realpath(input);
    } catch (error) {
      throw raise(input, `realpath failed: ${describe(error)}`);
    }
    if (real !== root) throw raise(input, `now resolves to ${real}, not ${root}`);
    return real;
  };
  captureRoot();
  const rootRecord = L(root);
  if (!rootRecord.ok || !rootRecord.stats.isDirectory() || rootRecord.identity !== rootIdentity) {
    throw raise(root, "is not the directory the checker was created for");
  }

  const loop = path => {
    const error = loopError(`${path}: too many levels of symbolic links`);
    loops.add(error);
    return error;
  };

  /** Kernel-style resolution of one syscall's path; throws the errno. */
  const resolve = (spelling, followFinal) => {
    if (typeof spelling !== "string" || spelling.includes("\0")) throw raise(String(spelling), "is not a path string");
    const path = spelling.startsWith("/") ? spelling : `/${spelling}`;
    const pending = path.split("/").filter(component => component !== "");
    if (path.length > 1 && path.endsWith("/")) pending.push(".");
    let current = "/";
    let substitutions = 0;
    const finish = (at, lstat) => ({ path: at, lstat });
    while (pending.length > 0) {
      const name = pending.shift();
      const last = pending.length === 0;
      if (name === "." || name === "..") {
        if (name === "..") current = posix.dirname(current);
        if (last) return finish(current, L(current));
        continue;
      }
      const next = current === "/" ? `/${name}` : `${current}/${name}`;
      const lstat = L(next);
      if (!lstat.ok) throw expected(lstat.code);
      if (lstat.stats.isSymbolicLink() && (!last || followFinal)) {
        if (++substitutions > links) throw loop(path);
        const target = R(next);
        if (!target.ok) throw raise(next, `is a link whose target cannot be read (${target.code})`);
        const components = target.text.split("/").filter(component => component !== "");
        if (target.text === "") throw expected("ENOENT");
        if (target.text.endsWith("/") && components.length > 0) components.push(".");
        if (target.text.startsWith("/")) current = "/";
        pending.unshift(...components);
        if (components.length === 0 && last) return finish(current, L(current));
        continue;
      }
      if (last) return finish(next, lstat);
      if (!lstat.stats.isDirectory()) throw expected("ENOTDIR");
      current = next;
    }
    return finish(current, L(current));
  };


  const fdEntry = fd => {
    const entry = fds.get(fd);
    if (!entry) throw expected("EBADF");
    return entry;
  };
  const readOnly = method => () => {
    guard();
    throw raise(method, "the owned directory is read-only");
  };
  const withFault = method => (...args) => {
    guard();
    try {
      return method(...args);
    } catch (error) {
      if (fault) throw fault;
      if (loops.has(error) || answers.has(error)) throw error;
      throw raise("fs", `unexpected ${describe(error)}`);
    }
  };

  const fs = {
    realpathSync: withFault(path => {
      if (path !== "/") throw raise(String(path), "realpathSync is only consumed for the preopen /");
      return "/";
    }),
    openSync: withFault((path, flags) => {
      const bits = flags === "r" ? 0 : flags;
      if (typeof bits !== "number" || !Number.isInteger(bits) || bits < 0) throw raise(String(path), `open flags ${String(flags)} are not consumed`);
      if ((bits & MODIFYING_FLAGS) !== 0) throw raise(String(path), `the owned directory is read-only: open flags ${bits} create, truncate or append`);
      if ((bits & ~(O_ACCMODE | O_EXCL | O_NONBLOCK | O_DIRECTORY | O_NOFOLLOW)) !== 0 || (bits & O_ACCMODE) === 3) {
        throw raise(String(path), `open flags ${bits} are not consumed`);
      }
      const { path: at, lstat } = resolve(path, (bits & O_NOFOLLOW) === 0);
      if (lstat.stats.isSymbolicLink()) throw loop(at);
      if ((bits & O_DIRECTORY) !== 0 && !lstat.stats.isDirectory()) throw expected("ENOTDIR");
      if (anchorAvailable && path === "/" && flags === "r") {
        anchorAvailable = false;
        if (!lstat.stats.isDirectory()) throw raise("/", "the preopen is not a directory");
        const fd = nextFd++;
        fds.set(fd, { path: at, stats: lstat.stats, bytes: null, directory: true, anchor: true });
        return fd;
      }
      const opened = F(at, lstat);
      if (!opened.ok) throw expected(opened.code);
      const { bytes } = opened;
      const fd = nextFd++;
      fds.set(fd, { path: at, stats: lstat.stats, bytes, directory: lstat.stats.isDirectory() });
      return fd;
    }),
    closeSync: withFault(fd => {
      fdEntry(fd);
      fds.delete(fd);
    }),
    fstatSync: withFault(fd => fdEntry(fd).stats),
    readSync: withFault((fd, buffer, offset, length, position) => {
      const entry = fdEntry(fd);
      if (!(buffer instanceof Uint8Array) || ![offset, length, position].every(Number.isSafeInteger) ||
          offset < 0 || length < 0 || position < 0 || offset + length > buffer.length) {
        throw raise(entry.path, "readSync arguments are not consumed");
      }
      if (entry.directory) throw expected("EISDIR");
      const bytes = entry.bytes.subarray(Math.min(position, entry.bytes.length), Math.min(position + length, entry.bytes.length));
      buffer.set(bytes, offset);
      return bytes.length;
    }),
    lstatSync: withFault(path => resolve(path, false).lstat.stats),
    statSync: withFault(path => {
      const { path: at, lstat } = resolve(path, true);
      if (lstat.stats.isSymbolicLink()) throw loop(at);
      return lstat.stats;
    }),
    readlinkSync: withFault((path, options) => {
      if (options !== undefined) throw raise(String(path), "readlinkSync options are not consumed");
      const { path: at } = resolve(path, false);
      const target = R(at);
      if (!target.ok) throw expected(target.code);
      return target.text;
    }),
    readdirSync: withFault((path, options) => {
      if (options?.withFileTypes !== true || Object.keys(options).length !== 1) {
        throw raise(String(path), "readdirSync options are not consumed");
      }
      const { path: at } = resolve(path, true);
      const listing = D(at);
      if (!listing.ok) throw expected(listing.code);
      return listing.entries.map(([name, type]) => dirent(name, type));
    }),
  };
  for (const method of ["writeSync", "ftruncateSync", "mkdirSync", "rmdirSync", "unlinkSync", "renameSync",
    "symlinkSync", "linkSync", "utimesSync", "futimesSync", "fsyncSync", "fdatasyncSync"]) {
    fs[method] = readOnly(method);
  }

  /** Re-performs every record; the first difference becomes the sticky fault. */
  const validate = () => {
    if (fault) return fault;
    const differs = (key, why) => raise(key.replace("\0", " "), `changed since it was read: ${why}`);
    try {
      captureRoot();
      for (const key of order) {
        const was = records.get(key);
        const [primitive, path] = key.split("\0");
        if (primitive === "L") {
          const now = captureL(path);
          if (now.ok !== was.ok || (now.ok ? now.identity !== was.identity : now.code !== was.code)) return differs(key, "lstat");
        } else if (primitive === "R") {
          const now = captureR(path);
          if (now.ok !== was.ok || (now.ok ? Buffer.compare(now.bytes, was.bytes) !== 0 : now.code !== was.code)) return differs(key, "readlink");
        } else if (primitive === "D") {
          const now = captureD(path);
          if (now.ok !== was.ok || now.identity !== was.identity ||
              (now.ok ? JSON.stringify(now.entries) !== JSON.stringify(was.entries) : now.code !== was.code)) return differs(key, "listing");
        } else if (was.ok) {
          if (identityNow(path) !== was.identity) return differs(key, "content identity");
        } else {
          const now = attempt("F", path, () => source.open(path));
          if (now.ok) {
            source.close(now.value);
            return differs(key, "now opens");
          }
          if (now.code !== was.code) return differs(key, "open error");
        }
      }
    } catch (error) {
      if (error instanceof OwnedInputFault) return fault;
      return raise(root, `validation failed: ${describe(error)}`);
    }
    return fault;
  };


  /** One record's outcome as a frozen copy, for the repository's tests only. */
  const observe = (primitive, path) => {
    const value = records.get(`${primitive}\0${path}`);
    if (!value) return undefined;
    return Object.freeze(value.ok ? { ok: true } : { ok: false, code: value.code });
  };

  /**
   * Constructs a WASI runtime over `fs`: its one open of the preopen `/` is
   * an anchor answered from `/`'s L record, with no host open. Only that
   * open, and only during `construct`.
   */
  const preopen = construct => {
    if (constructing) throw raise("/", "preopen windows do not nest");
    constructing = true;
    anchorAvailable = true;
    try {
      return construct();
    } finally {
      constructing = false;
      anchorAvailable = false;
    }
  };
  /** How many anchors are open, for the repository's tests only. */
  const anchors = () => [...fds.values()].filter(entry => entry.anchor).length;

  return Object.freeze({
    fs: Object.freeze(fs),
    /** The sticky fault, if any. */
    fault: () => fault,
    validate,
    /** The canonical root this store answers for. */
    root,
    observe,
    preopen,
    anchors,
    /** Record counts and held bytes, for measurement only. */
    metrics: () => {
      const counts = { L: 0, R: 0, D: 0, F: 0 };
      let bytes = 0;
      for (const [key, value] of records) {
        counts[key[0]]++;
        if (value.bytes && key[0] === "F") bytes += value.bytes.length;
      }
      return { records: records.size, counts, bytes, openFds: fds.size };
    },
  });
}
