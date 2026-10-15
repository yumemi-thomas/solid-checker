// The directory store (P5 S3b, B), on temporary trees: what it answers, what
// it records once, where it refuses, and that validation detects the changes
// it claims to detect. A spy source counts every host call.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import * as nodeFs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, test } from "vitest";
import { createDirectoryStore, hostSource, identityOf, OwnedInputFault } from "../owned-snapshot.mjs";
// The runtime's own bound, which the owned worker passes to the store.
import { MAX_SUBSTITUTIONS as LINKS } from "../vendor/wasm-util/lib/mjs/wasi/physical.mjs";

const trees = [];
afterEach(() => {
  for (const tree of trees.splice(0)) {
    execFileSync("chmod", ["-R", "u+rwx", tree]);
    nodeFs.rmSync(tree, { recursive: true, force: true });
  }
});

/** A fresh tree: `{ "a/b.txt": "text", "l": { link: "a" }, "d": {} }`. */
function tree(layout) {
  const top = nodeFs.realpathSync(nodeFs.mkdtempSync(join(tmpdir(), "owned-snapshot-")));
  trees.push(top);
  const root = join(top, "root");
  nodeFs.mkdirSync(root);
  for (const [path, value] of Object.entries(layout)) {
    const at = join(root, path);
    nodeFs.mkdirSync(join(at, ".."), { recursive: true });
    if (typeof value === "string") nodeFs.writeFileSync(at, value);
    else if (value.link !== undefined) nodeFs.symlinkSync(value.link, at);
    else nodeFs.mkdirSync(at, { recursive: true });
  }
  return { top, root };
}

function spy(overrides = {}) {
  const calls = [];
  const source = {};
  for (const name of Object.keys(hostSource)) {
    source[name] = (...args) => {
      calls.push([name, args[0]]);
      return (overrides[name] ?? hostSource[name])(...args);
    };
  }
  return { calls, source };
}

const loopError = message => Object.assign(new Error(message), { loop: true });

function open(root, options = {}) {
  const { calls, source } = spy(options.overrides);
  const store = createDirectoryStore({
    input: root, root, rootIdentity: identityOf(nodeFs.lstatSync(root, { bigint: true })),
    loopError, links: LINKS, source,
  });
  return { store, fs: store.fs, calls };
}

const code = expected => error => error?.code === expected;
const faulted = pattern => error => error instanceof OwnedInputFault && pattern.test(error.message);
const read = (fs, path) => {
  const fd = fs.openSync(path, 0, 0o666);
  const size = Number(fs.fstatSync(fd, { bigint: true }).size);
  const buffer = new Uint8Array(size + 4);
  const length = fs.readSync(fd, buffer, 0, buffer.length, 0);
  fs.closeSync(fd);
  return new TextDecoder().decode(buffer.subarray(0, length));
};

test("a file is captured once and answered from its record, with descriptors, positions and stats", () => {
  const { root } = tree({ "a.txt": "hello" });
  const { store, fs, calls } = open(root);
  const path = `${root}/a.txt`;
  const fd = fs.openSync(path, 0, 0o666);
  assert.ok(fd >= 2 ** 30);
  const stats = fs.fstatSync(fd, { bigint: true });
  assert.equal(stats.size, 5n);
  assert.equal(typeof stats.mtimeMs, "bigint");
  assert.equal(typeof stats.atimeMs, "bigint");
  assert.ok(stats.isFile() && !stats.isDirectory() && !stats.isSymbolicLink());
  const buffer = new Uint8Array(3);
  assert.equal(fs.readSync(fd, buffer, 0, 3, 2), 3);
  assert.equal(new TextDecoder().decode(buffer), "llo");
  assert.equal(fs.readSync(fd, buffer, 0, 3, 5), 0, "EOF");
  fs.closeSync(fd);
  assert.throws(() => fs.closeSync(fd), code("EBADF"));
  assert.throws(() => fs.fstatSync(1, { bigint: true }), code("EBADF"));
  const opens = calls.filter(([name]) => name === "open").length;
  assert.equal(read(fs, path), "hello");
  assert.equal(calls.filter(([name]) => name === "open").length, opens, "the second open reads no host file");
  assert.equal(store.validate(), undefined);
  assert.deepEqual(store.metrics().counts, { L: 2, R: 0, D: 0, F: 1 });
});

test("readlink of a regular file answers EINVAL as a recorded, revalidated outcome", () => {
  const { root } = tree({ "a.txt": "x" });
  const { store, fs, calls } = open(root);
  assert.throws(() => fs.readlinkSync(`${root}/a.txt`), code("EINVAL"));
  assert.throws(() => fs.readlinkSync(`${root}/a.txt`), code("EINVAL"));
  assert.equal(calls.filter(([name]) => name === "readlink").length, 1);
  assert.equal(store.validate(), undefined);
  assert.equal(calls.filter(([name]) => name === "readlink").length, 2, "validation re-performs the readlink");
});

test("a dangling link and an absent path are different outcomes, each revalidated through its own records", () => {
  const { root } = tree({ dangling: { link: "missing.txt" } });
  const { store, fs } = open(root);
  assert.ok(fs.lstatSync(`${root}/dangling`, { bigint: true }).isSymbolicLink());
  assert.equal(fs.readlinkSync(`${root}/dangling`), "missing.txt");
  assert.throws(() => fs.statSync(`${root}/dangling`, { bigint: true }), code("ENOENT"));
  assert.throws(() => fs.openSync(`${root}/dangling`, 0, 0o666), code("ENOENT"));
  assert.throws(() => fs.lstatSync(`${root}/absent`, { bigint: true }), code("ENOENT"));
  assert.equal(store.validate(), undefined);
  nodeFs.writeFileSync(`${root}/missing.txt`, "now here");
  // The first changed witness in capture order names it: the root's listing
  // identity, or the absent path itself.
  assert.throws(() => { throw store.validate(); }, faulted(/(root|missing\.txt): changed since it was read: lstat/));
});

test(`links resolve physically, ${LINKS} substitutions resolve and one more is ELOOP`, () => {
  const layout = { "real/sub/x.txt": "physical", "real/x.txt": "real", "x.txt": "lexical", alias: { link: "real/sub" } };
  for (let index = 0; index < LINKS; index++) layout[`chain${index}`] = { link: index === LINKS - 1 ? "real/x.txt" : `chain${index + 1}` };
  layout.over = { link: "chain0" };
  layout.self = { link: "self" };
  const { root } = tree(layout);
  const { fs } = open(root);
  assert.equal(read(fs, `${root}/alias/x.txt`), "physical");
  assert.equal(read(fs, `${root}/alias/../x.txt`), "real", "`..` after a link is the parent of the directory reached");
  assert.equal(read(fs, `${root}/chain0`), "real");
  assert.throws(() => fs.statSync(`${root}/over`, { bigint: true }), error => error.loop === true);
  assert.throws(() => fs.openSync(`${root}/self`, 0, 0o666), error => error.loop === true);
  assert.throws(() => fs.openSync(`${root}/alias`, 131072, 0o666), error => error.loop === true, "O_NOFOLLOW on a link");
  assert.throws(() => fs.readdirSync(`${root}/real/x.txt/`, { withFileTypes: true }), code("ENOTDIR"));
});

test("a store without the runtime's loop error and link bound refuses to start", () => {
  const { root } = tree({});
  const rootIdentity = identityOf(nodeFs.lstatSync(root, { bigint: true }));
  for (const missing of [{ loopError: undefined }, { links: undefined }, { links: 0 }, { links: 1.5 }]) {
    assert.throws(() => createDirectoryStore({ input: root, root, rootIdentity, loopError, links: LINKS, ...missing }),
      faulted(/needs the WASI runtime's loop error and link bound/), JSON.stringify(missing));
  }
});

test("a link leading outside the root faults when it is reached, and the fault is sticky", () => {
  const { top, root } = tree({ "a.txt": "x", escape: { link: "../outside" }, back: { link: "../root/a.txt" } });
  nodeFs.mkdirSync(join(top, "outside"));
  nodeFs.writeFileSync(join(top, "outside/secret.txt"), "secret");
  const { store, fs, calls } = open(root);
  assert.deepEqual(fs.readdirSync(root, { withFileTypes: true }).map(entry => [entry.name, entry.isSymbolicLink()]),
    [["a.txt", false], ["back", true], ["escape", true]]);
  assert.ok(fs.lstatSync(`${root}/escape`, { bigint: true }).isSymbolicLink(), "an unfollowed escaping link answers");
  assert.equal(read(fs, `${root}/back`), "x", "a link that leaves through an ancestor and returns is inside");
  assert.throws(() => fs.statSync(`${root}/escape/secret.txt`, { bigint: true }), faulted(/a link leads outside the directory root/));
  const before = calls.length;
  assert.throws(() => read(fs, `${root}/a.txt`), error => error === store.fault());
  assert.throws(() => fs.realpathSync("/", "utf8"), error => error === store.fault());
  assert.equal(calls.length, before, "no host call after the fault");
  assert.equal(store.validate(), store.fault());
  assert.ok(!calls.some(([, path]) => typeof path === "string" && path.includes("outside/secret")));
});

test("a guest reading a link whose target leaves the root faults, even if it would follow the link itself", () => {
  const { top, root } = tree({ inside: { link: "missing/../a.txt" }, dangling: { link: "nowhere" }, escape: { link: "../outside" },
    absolute: { link: "/etc/hosts" }, through: { link: "../root/a.txt" }, "a.txt": "x" });
  const { fs } = open(root);
  assert.equal(fs.readlinkSync(`${root}/dangling`), "nowhere", "a target missing inside the root is only dangling");
  assert.equal(fs.readlinkSync(`${root}/through`), "../root/a.txt", "a target through an ancestor and back is inside");
  assert.equal(fs.readlinkSync(`${root}/inside`), "missing/../a.txt");
  assert.throws(() => fs.readlinkSync(`${root}/escape`), faulted(/a link leads outside the directory root/));
  const absolute = open(root);
  assert.throws(() => absolute.fs.readlinkSync(`${root}/absolute`), faulted(/a link leads outside the directory root/));
  assert.ok(!nodeFs.existsSync(join(top, "outside")), "the escaping target does not even exist: it still faults");
});

test("ancestors are virtual, nothing else outside the root is read, and only / is realpath'd", () => {
  const { top, root } = tree({ "a.txt": "x" });
  nodeFs.writeFileSync(join(top, "sibling.txt"), "sibling");
  const { fs, calls } = open(root);
  const parent = fs.lstatSync(top, { bigint: true });
  assert.ok(parent.isDirectory());
  assert.deepEqual(fs.readdirSync(top, { withFileTypes: true }).map(entry => entry.name), ["root"]);
  assert.ok(fs.statSync("/", { bigint: true }).isDirectory());
  assert.throws(() => fs.lstatSync(join(top, "sibling.txt"), { bigint: true }), code("ENOENT"));
  assert.throws(() => fs.openSync(`${root}/../sibling.txt`, 0, 0o666), code("ENOENT"));
  assert.equal(fs.realpathSync("/", "utf8"), "/");
  assert.ok(calls.every(([, path]) => typeof path !== "string" || path === root || path.startsWith(`${root}/`)), JSON.stringify(calls));
  assert.throws(() => fs.realpathSync(root, "utf8"), faulted(/realpathSync is only consumed for the preopen/));
});

test("permission errors pass through as the host's errno, are recorded, and a later change is detected", () => {
  const { root } = tree({ "locked.txt": "x", "closed/a.txt": "y", "unsearchable/b.txt": "z" });
  nodeFs.chmodSync(`${root}/locked.txt`, 0o000);
  nodeFs.chmodSync(`${root}/closed`, 0o000);
  nodeFs.chmodSync(`${root}/unsearchable`, 0o600);
  const { store, fs } = open(root);
  assert.throws(() => fs.openSync(`${root}/locked.txt`, 0, 0o666), code("EACCES"));
  assert.throws(() => fs.readdirSync(`${root}/closed`, { withFileTypes: true }), code("EACCES"));
  assert.throws(() => fs.lstatSync(`${root}/unsearchable/b.txt`, { bigint: true }), code("EACCES"));
  assert.equal(store.validate(), undefined);
  nodeFs.chmodSync(`${root}/locked.txt`, 0o644);
  assert.throws(() => { throw store.validate(); }, faulted(/locked\.txt: changed since it was read/));
});

test("a FIFO is an input fault", () => {
  const { root } = tree({});
  execFileSync("mkfifo", [`${root}/pipe`]);
  const { fs } = open(root);
  assert.throws(() => fs.lstatSync(`${root}/pipe`, { bigint: true }), faulted(/is not a regular file, directory or link/));
});

test("a name or link target that is not UTF-8 is an input fault", () => {
  const { root } = tree({ link: { link: "a" } });
  const invalid = Buffer.from([0x61, 0xff]);
  const named = open(root, { overrides: { readdir: () => [{ name: new Uint8Array(invalid), type: "file" }] } });
  assert.throws(() => named.fs.readdirSync(root, { withFileTypes: true }), faulted(/lists a name that is not UTF-8/));
  const targeted = open(root, { overrides: { readlink: () => invalid } });
  assert.throws(() => targeted.fs.readlinkSync(`${root}/link`), faulted(/link target is not UTF-8/));
});

// A leading U+FEFF (bytes EF BB BF) is part of a filesystem name, not an
// encoding mark: decoding must keep it, or a link to U+FEFF + "target.ts" is
// followed to "target.ts" and validation cannot notice.
const BOM = String.fromCharCode(0xfeff);

test("a link target beginning with U+FEFF is read, followed and validated as that exact name", () => {
  const { root } = tree({ [`${BOM}target.ts`]: "RIGHT", "target.ts": "WRONG", "alias.ts": { link: `${BOM}target.ts` }, "plain.ts": { link: "target.ts" } });
  assert.deepEqual([...nodeFs.readlinkSync(`${root}/alias.ts`, { encoding: "buffer" }).subarray(0, 3)], [0xef, 0xbb, 0xbf]);
  const { store, fs } = open(root);
  assert.equal(fs.readlinkSync(`${root}/alias.ts`), `${BOM}target.ts`);
  assert.equal(read(fs, `${root}/alias.ts`), "RIGHT");
  assert.equal(fs.readlinkSync(`${root}/plain.ts`), "target.ts");
  assert.equal(read(fs, `${root}/plain.ts`), "WRONG");
  assert.equal(read(fs, `${root}/${BOM}target.ts`), "RIGHT");
  assert.equal(store.validate(), undefined);
});

test("a listing keeps a name beginning with U+FEFF beside the same name without it, in raw-byte order", () => {
  const { root } = tree({ [`${BOM}target.ts`]: "RIGHT", "target.ts": "WRONG", "alias.ts": { link: `${BOM}target.ts` } });
  const { store, fs } = open(root);
  const listing = fs.readdirSync(root, { withFileTypes: true }).map(entry => [entry.name, entry.isFile(), entry.isSymbolicLink()]);
  assert.deepEqual(listing, [["alias.ts", false, true], ["target.ts", true, false], [`${BOM}target.ts`, true, false]]);
  assert.equal(store.validate(), undefined);
});

test("writes are refused before any host access; an access mode alone opens the host read-only", () => {
  const { root } = tree({ "a.txt": "keep" });
  for (const flags of [64, 512, 1024, 1052672, 64 | 2]) {
    const { fs, calls } = open(root);
    const before = calls.length;
    assert.throws(() => fs.openSync(`${root}/a.txt`, flags, 0o666), faulted(/read-only: open flags \d+ create, truncate or append/), String(flags));
    assert.equal(calls.length, before, `flags ${flags} reached the host`);
  }
  for (const method of ["writeSync", "ftruncateSync", "mkdirSync", "rmdirSync", "unlinkSync", "renameSync", "symlinkSync",
    "linkSync", "utimesSync", "futimesSync", "fsyncSync", "fdatasyncSync"]) {
    const { fs, calls } = open(root);
    const before = calls.length;
    assert.throws(() => fs[method](`${root}/a.txt`), faulted(/the owned directory is read-only/), method);
    assert.equal(calls.length, before);
  }
  for (const flags of [1, 2, 2 | 2048]) {
    const { fs } = open(root);
    assert.equal(read({ ...fs, openSync: path => fs.openSync(path, flags, 0o666) }, `${root}/a.txt`), "keep");
  }
  const { fs } = open(root);
  assert.throws(() => fs.openSync(`${root}/a.txt`, 3, 0o666), faulted(/open flags 3 are not consumed/));
  assert.equal(nodeFs.readFileSync(`${root}/a.txt`, "utf8"), "keep");
});

test("an unexpected host error or a file changing while it is read faults, and the host descriptor is closed", () => {
  const { root } = tree({ "a.txt": "abc", "b.txt": "def", "c.txt": "ghi" });
  const eio = open(root, { overrides: { read: () => { throw Object.assign(new Error("EIO"), { code: "EIO" }); } } });
  assert.throws(() => eio.fs.openSync(`${root}/a.txt`, 0, 0o666), faulted(/F failed: EIO/));
  let stats = 0;
  const changing = open(root, { overrides: { fstat: fd => {
    const value = hostSource.fstat(fd);
    return ++stats === 2 ? { ...value, size: value.size + 1n, isFile: () => true } : value;
  } } });
  assert.throws(() => changing.fs.openSync(`${root}/b.txt`, 0, 0o666), faulted(/changed while it was read/));
  const emfile = open(root, { overrides: { open: () => { throw Object.assign(new Error("EMFILE"), { code: "EMFILE" }); } } });
  assert.throws(() => emfile.fs.openSync(`${root}/c.txt`, 0, 0o666), faulted(/F failed: EMFILE/));
  for (const { calls } of [eio, changing]) {
    assert.equal(calls.filter(([name]) => name === "open").length, calls.filter(([name]) => name === "close").length);
  }
});

test("a listing that changes while it is taken faults", () => {
  const { root } = tree({ "d/a.txt": "x" });
  const { fs } = open(root, { overrides: { readdir: path => {
    const entries = hostSource.readdir(path);
    nodeFs.writeFileSync(`${root}/d/late.txt`, "late");
    return entries;
  } } });
  assert.throws(() => fs.readdirSync(`${root}/d`, { withFileTypes: true }), faulted(/changed while it was listed/));
});

test("validation detects a changed file, an added entry, a retargeted link, a replaced directory and a replaced root", () => {
  // Each fault names the first changed witness in capture order: the changed
  // entry, or a directory whose identity its change also altered.
  const cases = {
    content: [root => nodeFs.writeFileSync(`${root}/a.txt`, "changed!"), /a\.txt: changed since it was read/],
    entry: [root => nodeFs.writeFileSync(`${root}/d/new.txt`, ""), /d: changed since it was read/],
    link: [root => { nodeFs.unlinkSync(`${root}/l`); nodeFs.symlinkSync("d/b.txt", `${root}/l`); }, /(root|l): changed since it was read/],
    intermediate: [root => { nodeFs.renameSync(`${root}/d`, `${root}/d.old`); nodeFs.symlinkSync("d.old", `${root}/d`); }, /(root|d): changed since it was read/],
    root: [root => { nodeFs.renameSync(root, `${root}.old`); nodeFs.mkdirSync(root); }, /root: changed since it was read|is not the directory/],
  };
  for (const [name, [change, pattern]] of Object.entries(cases)) {
    const { root } = tree({ "a.txt": "original", "d/b.txt": "b", l: { link: "a.txt" } });
    const { store, fs } = open(root);
    assert.equal(read(fs, `${root}/l`), "original");
    assert.equal(read(fs, `${root}/d/b.txt`), "b");
    fs.readdirSync(`${root}/d`, { withFileTypes: true });
    assert.equal(store.validate(), undefined, name);
    change(root);
    const fault = store.validate();
    assert.ok(fault instanceof OwnedInputFault && pattern.test(fault.message), `${name}: ${fault?.message}`);
    assert.throws(() => fs.lstatSync(root, { bigint: true }), error => error === fault, "and it is sticky");
  }
});

test("a store refuses a root that is not the directory it was created for", () => {
  const { root } = tree({});
  const identity = identityOf(nodeFs.lstatSync(root, { bigint: true }));
  nodeFs.rmdirSync(root);
  nodeFs.mkdirSync(root);
  assert.throws(() => createDirectoryStore({ input: root, root, rootIdentity: identity, loopError, links: LINKS }),
    faulted(/is not the directory the checker was created for/));
  assert.throws(() => createDirectoryStore({ input: root, root: "/", rootIdentity: identity, loopError, links: LINKS }), OwnedInputFault);
});

test("listing order is by name bytes on every engine, types are the host's, and both are fixed by the record", () => {
  const { root } = tree({ "b.txt": "", "A.txt": "", "c/x": "", "a": { link: "b.txt" } });
  const { fs } = open(root);
  const first = fs.readdirSync(root, { withFileTypes: true }).map(entry => [entry.name, entry.isFile(), entry.isDirectory(), entry.isSymbolicLink()]);
  assert.deepEqual(first.map(([name]) => name), ["A.txt", "a", "b.txt", "c"]);
  assert.deepEqual(first.find(([name]) => name === "a"), ["a", false, false, true]);
  nodeFs.writeFileSync(`${root}/zz.txt`, "");
  assert.deepEqual(fs.readdirSync(root, { withFileTypes: true }).map(entry => [entry.name, entry.isFile(), entry.isDirectory(), entry.isSymbolicLink()]), first);
});
