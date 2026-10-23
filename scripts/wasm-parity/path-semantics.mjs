// The WASM parity gate's path-semantics controls (S3a): real WASI path calls,
// made by two control guests through a given WASI runtime over a memfs copy
// of a disk tree, against host POSIX on that same tree.
//
// The rows, the disk shapes and the host reference live here; the gate runs
// the guests through the owned runtime (vendor/wasm-util) and, as the
// old-layer control, through the pinned upstream @tybys/wasm-util.
import {
  closeSync, constants, cpSync, fstatSync, lstatSync, mkdirSync, mkdtempSync, openSync, readdirSync, readlinkSync,
  realpathSync, rmdirSync, statSync, symlinkSync, unlinkSync, writeFileSync,
} from "node:fs";
import { join } from "node:path";

const LONG_TARGET = `long/${"segment-".repeat(40)}end`;
export const WASI_ERRNO = { ENOENT: 44, ENOTDIR: 54, ELOOP: 32, EEXIST: 20, EISDIR: 31, EINVAL: 28, ENOTEMPTY: 55, EACCES: 2, EPERM: 63, EBADF: 8 };

/** The shapes every row runs over, created under `root` on disk. */
export function buildTree(root) {
  const file = (path, text) => {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), text);
  };
  file("a/b/c/d/file.txt", "depth\n");
  mkdirSync(join(root, "a/b/c/d/e"), { recursive: true });
  file("real/inner/x.txt", "x\n");
  file("real/sibling.txt", "sibling\n");
  mkdirSync(join(root, "real/empty"), { recursive: true });
  file("f", "f\n");
  file("store/node_modules/.pnpm/pkg@1.0.0/node_modules/pkg/index.js", "export {};\n");
  file("store/node_modules/.pnpm/pkg@1.0.0/node_modules/pkg/lib/util.js", "export {};\n");
  symlinkSync("real/inner", join(root, "linkdir"));
  symlinkSync("f", join(root, "filelink"));
  symlinkSync(join(root, "real/sibling.txt"), join(root, "abslink"));
  symlinkSync("chain2", join(root, "chain1"));
  symlinkSync("chain3", join(root, "chain2"));
  symlinkSync("real/sibling.txt", join(root, "chain3"));
  symlinkSync("missing-target", join(root, "dangling"));
  symlinkSync("loop2", join(root, "loop1"));
  symlinkSync("loop1", join(root, "loop2"));
  symlinkSync(".", join(root, "again"));
  symlinkSync(LONG_TARGET, join(root, "longlink"));
  symlinkSync(".pnpm/pkg@1.0.0/node_modules/pkg", join(root, "store/node_modules/pkg"));
  // Stored targets ending in `/`: the name before the slash must resolve to a
  // directory, wherever the link is met (CODE R1).
  symlinkSync("f/", join(root, "slashfile"));
  symlinkSync("real/", join(root, "slashdir"));
  symlinkSync("slashfile", join(root, "slashchain"));
  symlinkSync("filelink/", join(root, "slashvia"));
  symlinkSync("newdir-target/", join(root, "slashmissing"));
}

// A row: { id, op, dirfd, lookup, oflags, fdflags, write, path, buflen, check }
// with `path` relative to the tree root (or to the dirfd for `dir=`/`file=`/`closed=`).
const LOOKUP_PATHS = [
  "a/b/c/d/e/../file.txt", "a/b/c/d/e/../../d/file.txt", "a/b/c/d/e/../../../c/d/file.txt",
  "a/b/c/d/e/../../../../b/c/d/file.txt", "linkdir/../sibling.txt", "linkdir/x.txt", "abslink",
  "chain1", "dangling", "loop1", "again/again/f", "f/.", "f/..", "missing/..", "f/", "real/", "linkdir/",
  "filelink/", "f/x", "linkdir", "filelink", "store/node_modules/pkg/lib/../index.js",
  "store/node_modules/pkg/lib/util.js", "missing", "a/b/c/d/e/",
  "slashfile", "slashdir", "slashdir/sibling.txt", "slashchain", "slashvia", "slashmissing",
];
const LOOKUP_OPS = [
  ["stat-follow", { op: "stat", lookup: 1 }], ["stat-nofollow", { op: "stat", lookup: 0 }],
  ["open-follow", { op: "open", lookup: 1 }], ["open-nofollow", { op: "open", lookup: 0 }],
  ["opendir-follow", { op: "open", lookup: 1, oflags: 2 }], ["opendir-nofollow", { op: "open", lookup: 0, oflags: 2 }],
  ["readlink", { op: "readlink", buflen: 512 }],
];

export function rows() {
  const base = { dirfd: "pre", lookup: 0, oflags: 0, fdflags: 0, write: 0, buflen: 0 };
  const out = [];
  for (const path of LOOKUP_PATHS) {
    for (const [name, spec] of LOOKUP_OPS) out.push({ ...base, ...spec, id: `${name}:${path}`, path });
  }
  const longLength = Buffer.byteLength(LONG_TARGET);
  for (const buflen of [0, 1, longLength - 1, longLength, longLength + 16]) {
    out.push({ ...base, id: `readlink-buffer:${buflen}`, op: "readlink", path: "longlink", buflen });
  }
  out.push({ ...base, id: "readlink-buffer:chain1:5", op: "readlink", path: "chain1", buflen: 5 });
  for (const [dirfd, path, op, lookup] of [
    ["dir=a/b/c", "d/file.txt", "stat", 1], ["dir=a/b/c", "../c/d/file.txt", "stat", 1],
    ["dir=a/b/c", "../../../real/sibling.txt", "stat", 1], ["dir=linkdir", "../sibling.txt", "stat", 1],
    ["dir=linkdir", "x.txt", "stat", 1], ["dir=linkdir", "../sibling.txt", "open", 1],
    ["file=f", "x", "stat", 1], ["closed=real", "sibling.txt", "stat", 1],
    ["dir=slashdir", "sibling.txt", "stat", 1],
  ]) out.push({ ...base, id: `dirfd:${dirfd}:${op}:${path}`, dirfd, path, op, lookup });
  out.push({ ...base, id: "spot:setflags", op: "setflags", path: "" });
  out.push({ ...base, id: "spot:readdir", op: "readdir", dirfd: "dir=real", path: "" });
  return out;
}

/** Rows that change the tree, each run against its own fresh copy. */
export function mutationRows() {
  const base = { dirfd: "pre", lookup: 1, oflags: 0, fdflags: 0, write: 1, buflen: 0 };
  return [
    { op: "mkdir", path: "real/inner/../newdir", check: "real/newdir" },
    { op: "mkdir", path: "linkdir/../newdir2", check: "real/newdir2" },
    { op: "mkdir", path: "real/newdir3/", check: "real/newdir3" },
    { op: "mkdir", path: "f", check: "f" },
    { op: "rmdir", path: "linkdir/../empty", check: "real/empty" },
    { op: "rmdir", path: "real/empty/", check: "real/empty" },
    { op: "unlink", path: "linkdir/../sibling.txt", check: "real/sibling.txt" },
    { op: "unlink", path: "f/", check: "f" },
    { op: "unlink", path: "filelink", check: "filelink" },
    { op: "open", oflags: 1, path: "linkdir/../created.txt", check: "real/created.txt" },
    { op: "open", oflags: 1 | 4, path: "filelink", check: "f" },
    { op: "open", oflags: 1 | 4, path: "dangling", check: "missing-target" },
    { op: "open", oflags: 1, path: "dangling", check: "missing-target" },
    { op: "open", oflags: 8, path: "linkdir/../sibling.txt", check: "real/sibling.txt" },
    { op: "open", fdflags: 1, path: "f", check: "f" },
    { op: "open", oflags: 1, path: "missing/new.txt", check: "missing" },
    // CREAT through a stored `dir/` target must not create a regular file.
    { op: "open", oflags: 1, path: "slashmissing", check: "newdir-target" },
    { op: "open", oflags: 1, path: "slashfile", check: "f" },
    { op: "open", oflags: 1 | 4, path: "slashmissing", check: "newdir-target" },
  ].map((row, index) => ({ ...base, ...row, id: `mutation:${index}:${row.op}:${row.path}` }));
}

/** Guest-stdlib rows: Rust std (through wasi-libc), by absolute path. */
export function stdlibRows(root) {
  return {
    rust: [
      ["std-meta", "linkdir/../sibling.txt"], ["std-meta", "a/b/c/d/e/../../../c/d/file.txt"],
      ["std-readlink", "longlink"], ["std-canon", "linkdir/../sibling.txt"],
    // Concatenated, never path.join: join would clean the `..` away lexically.
    ].map(([op, path]) => ({ id: `rust:${op}:${path}`, op, dirfd: "pre", lookup: 0, oflags: 0, fdflags: 0, write: 0, buflen: 0, path: `${root}/${path}` })),
  };
}

/** The argument a guest receives for a row; paths become preopen-relative. */
/**
 * Rows the old layer cannot run: following the `loop1 -> loop2 -> loop1`
 * cycle never returns in upstream memfs, so those rows are recorded as not
 * executed there rather than hang the gate.
 */
export const oldLayerRuns = row => !row.path.includes("loop1");

export function guestArgument(row, root) {
  const relativeTo = path => `${root.slice(1)}/${path}`;
  if (row.op.startsWith("std-")) return [row.id, row.op, "pre", 0, 0, 0, 0, row.path, 0].join("|");
  const dirfd = row.dirfd === "pre" ? "pre" : `${row.dirfd.split("=")[0]}=${relativeTo(row.dirfd.split("=")[1])}`;
  const path = row.dirfd === "pre" && row.op !== "setflags" && row.op !== "readdir" ? relativeTo(row.path) : row.path;
  return [row.id, row.op, dirfd, row.lookup, row.oflags, row.fdflags, row.write, path, row.buflen].join("|");
}

const typeOf = stat => (stat.isDirectory() ? 3 : stat.isSymbolicLink() ? 7 : stat.isFile() ? 4 : 0);
const errnoOf = error => WASI_ERRNO[error.code] ?? `host:${error.code}`;

/** What host POSIX answers for a row over the disk tree at `root`. */
export function hostRow(row, root) {
  const at = !row.dirfd || row.dirfd === "pre" ? root : realpathSync(join(root, row.dirfd.split("=")[1]));
  const path = `${at}/${row.path}`;
  const check = () => {
    if (!row.check) return "";
    try {
      return ` check=${typeOf(lstatSync(join(root, row.check)))}`;
    } catch {
      return " check=missing";
    }
  };
  try {
    switch (row.op) {
      case "stat": {
        const stat = row.lookup ? statSync(path) : lstatSync(path);
        // Size only for regular files: directory sizes are filesystem-specific,
        // and memfs reports a link's size as 0 (retained memfs behaviour).
        return { errno: 0, detail: `type=${typeOf(stat)}${typeOf(stat) === 4 ? ` size=${stat.size}` : ""}` };
      }
      case "open": {
        let flags = row.write ? constants.O_RDWR : constants.O_RDONLY;
        if (row.oflags & 1) flags |= constants.O_CREAT;
        if (row.oflags & 2) flags |= constants.O_DIRECTORY;
        if (row.oflags & 4) flags |= constants.O_EXCL;
        if (row.oflags & 8) flags |= constants.O_TRUNC;
        if (row.fdflags & 1) flags |= constants.O_APPEND;
        if (!row.lookup) flags |= constants.O_NOFOLLOW;
        const fd = openSync(path, flags, 0o666);
        const type = typeOf(fstatSync(fd));
        closeSync(fd);
        return { errno: 0, detail: `type=${type}${check()}` };
      }
      case "readlink": {
        const target = readlinkSync(path, { encoding: "buffer" });
        const placed = Math.min(target.length, row.buflen);
        return { errno: 0, detail: `used=${placed} bytes=${target.subarray(0, placed).toString("hex")} before=true after=true untouched=true` };
      }
      case "mkdir": mkdirSync(path); return { errno: 0, detail: check().trim() };
      case "rmdir": rmdirSync(path); return { errno: 0, detail: check().trim() };
      case "unlink": unlinkSync(path); return { errno: 0, detail: check().trim() };
      case "readdir": return { errno: 0, detail: readdirSync(at).sort().join(",") };
      case "std-meta": {
        const stat = statSync(row.path);
        return { errno: 0, detail: `dir=${stat.isDirectory()} file=${stat.isFile()}` };
      }
      case "std-readlink": return { errno: 0, detail: `len=${readlinkSync(row.path, { encoding: "buffer" }).length}` };
      // The OS realpath(3): Node's JavaScript realpathSync cleans `..` first.
      case "std-canon": return { errno: 0, detail: realpathSync.native(row.path) };
      default: return { errno: "host:unsupported", detail: "" };
    }
  } catch (error) {
    // A failed readlink leaves the caller's buffer and its sentinels alone.
    if (row.op === "readlink") return { errno: errnoOf(error), detail: "before=true after=true" };
    if (row.op === "stat" || row.op.startsWith("std-")) return { errno: errnoOf(error), detail: "" };
    return { errno: errnoOf(error), detail: check().trim() };
  }
}

/** A fresh disk copy of `root` for one mutation row, beside it. */
export function copyTree(root, label) {
  const copy = mkdtempSync(`${root}-${label}-`);
  cpSync(root, copy, { recursive: true, verbatimSymlinks: true });
  // The one absolute link names its own tree; point the copy's at the copy.
  unlinkSync(join(copy, "abslink"));
  symlinkSync(join(copy, "real/sibling.txt"), join(copy, "abslink"));
  return copy;
}

/**
 * Runs one guest with `rowArguments` through `WASI` (a @tybys/wasm-util-shaped
 * class) over a memfs materialized from `mount`; returns `id -> {errno, detail}`.
 */
export function runGuest({ WASI, memfs, materializeMount, mount, module, rowArguments, checks }) {
  const { fs, vol } = memfs({}, "/");
  materializeMount(vol, mount);
  const lines = [];
  let buffered = "";
  const print = text => {
    buffered += text.endsWith("\n") ? text : `${text}\n`;
  };
  // returnOnExit: the guest's proc_exit returns from start() instead of
  // ending the host process.
  const wasi = new WASI({
    version: "preview1", args: ["control", ...rowArguments], env: {}, fs, preopens: { "/": "/" },
    print, printErr: () => {}, returnOnExit: true,
  });
  const instance = new WebAssembly.Instance(module, wasi.getImportObject());
  try {
    wasi.start(instance);
  } catch (error) {
    lines.push(`!trap|${error?.message ?? error}`);
  }
  lines.push(...buffered.split("\n").filter(Boolean));
  const results = {};
  for (const line of lines) {
    const [id, errno, ...detail] = line.split("|");
    results[id] = { errno: Number.isNaN(Number(errno)) ? errno : Number(errno), detail: detail.join("|") };
  }
  for (const [id, path] of Object.entries(checks ?? {})) {
    let state;
    try {
      const stat = fs.lstatSync(path);
      state = stat.isDirectory() ? 3 : stat.isSymbolicLink() ? 7 : stat.isFile() ? 4 : 0;
    } catch {
      state = "missing";
    }
    if (results[id]) results[id].detail = `${results[id].detail ? `${results[id].detail} ` : ""}check=${state}`.trim();
  }
  return results;
}
