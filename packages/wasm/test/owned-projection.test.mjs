// The project projection (P5 S3c, G3): the answer encoder against its store
// root, the three `solid_owned_host` imports and their pending state, the
// reactors' import inventory, and the directory store's walk -- each outcome
// on temporary trees, host calls counted by a spy source. Injected host
// errors establish routing only; the real-host cases below say so.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import * as nodeFs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, test } from "vitest";
import { coveredImports, OwnedBoundaryError } from "../owned-boundary.mjs";
import {
  checkImportInventory, cleanAbsolute, createProjectionHost, encodeProjection, OwnedProjectionError, TAGS,
} from "../owned-projection.mjs";
import { createDirectoryStore, hostSource, identityOf, OwnedInputFault } from "../owned-snapshot.mjs";
import { MAX_SUBSTITUTIONS as LINKS } from "../vendor/wasm-util/lib/mjs/wasi/physical.mjs";

const trees = [];
afterEach(() => {
  for (const tree of trees.splice(0)) {
    execFileSync("chmod", ["-R", "u+rwx", tree]);
    nodeFs.rmSync(tree, { recursive: true, force: true });
  }
});

/** `top/root` plus `layout` relative to `top`: strings are files, `{ link }` links, `{}` directories. */
function tree(layout) {
  const top = nodeFs.realpathSync(nodeFs.mkdtempSync(join(tmpdir(), "owned-projection-")));
  trees.push(top);
  const root = join(top, "root");
  nodeFs.mkdirSync(root);
  for (const [path, value] of Object.entries(layout)) {
    const at = join(top, path);
    nodeFs.mkdirSync(join(at, ".."), { recursive: true });
    if (typeof value === "string") nodeFs.writeFileSync(at, value);
    else if (value.link !== undefined) nodeFs.symlinkSync(value.link, at);
    else nodeFs.mkdirSync(at, { recursive: true });
  }
  return { top, root };
}

const project = { "root/project/tsconfig.json": "{}\n" };

function open(root, overrides = {}) {
  const calls = [];
  const source = {};
  for (const name of Object.keys(hostSource)) {
    source[name] = (...args) => {
      calls.push([name, args[0]]);
      return (overrides[name] ?? hostSource[name])(...args);
    };
  }
  const store = createDirectoryStore({
    input: root, root, rootIdentity: identityOf(nodeFs.lstatSync(root, { bigint: true })),
    loopError: message => Object.assign(new Error(message), { loop: true }), links: LINKS, source,
  });
  return { store, calls };
}

const enoent = () => Object.assign(new Error("ENOENT"), { code: "ENOENT" });
const coded = code => Object.assign(new Error(code), { code });
const bytes = (...values) => Uint8Array.from(values);
const utf8 = text => [...new TextEncoder().encode(text)];

// --- the encoder ------------------------------------------------------------------

test("every answer encodes to its tag and exact payload, checked against the store root", () => {
  const root = "/r";
  assert.deepEqual(encodeProjection({ tag: "unavailable" }), bytes(0));
  assert.deepEqual(encodeProjection({ tag: "not-outside" }, root), bytes(1));
  assert.deepEqual(encodeProjection({ tag: "faulted" }, root), bytes(6));
  assert.deepEqual(encodeProjection({ tag: "projected", path: "/r/p" }, root), bytes(2, ...utf8("/r/p")));
  assert.deepEqual(encodeProjection({ tag: "outside-root", path: "/o/t", root }, root), bytes(3, 4, 0, 0, 0, ...utf8("/o/t/r")));
  assert.deepEqual(encodeProjection({ tag: "outside-root", path: "/", root }, root), bytes(3, 1, 0, 0, 0, ...utf8("//r")));
  assert.deepEqual(encodeProjection({ tag: "name-variant", path: "/r/ACTUAL" }, root), bytes(4, ...utf8("/r/ACTUAL")));
  assert.deepEqual(encodeProjection({ tag: "host-error", errno: 32 }, root), bytes(5, 32, 0));
  assert.deepEqual(encodeProjection({ tag: "listing-unavailable", errno: 2, directory: "/r" }, root), bytes(7, 2, 0, ...utf8("/r")));
  assert.deepEqual(encodeProjection({ tag: "projected", path: "/r/\u{feff}p" }, root), bytes(2, ...utf8("/r/\u{feff}p")));
  assert.equal(Object.keys(TAGS).length, 8);
});

test("an answer the store could not have given is refused, an injected one included", () => {
  const root = "/r";
  const refused = [
    [{ tag: "projected", path: "/rr/p" }, root], [{ tag: "projected", path: "/o/p" }, root],
    [{ tag: "projected", path: "/r/p" }, undefined],
    [{ tag: "outside-root", path: "/r/p", root }, root], [{ tag: "outside-root", path: "/o", root: "/q" }, root],
    [{ tag: "name-variant", path: "/r" }, root], [{ tag: "name-variant", path: "/o/x" }, root],
    [{ tag: "listing-unavailable", errno: 2, directory: "/o" }, root],
    [{ tag: "listing-unavailable", errno: 2, directory: "/rr" }, root],
    [{ tag: "listing-unavailable", errno: 32, directory: "/r" }, root],
    [{ tag: "host-error", errno: 8 }, root], [{ tag: "host-error", errno: "44" }, root],
    [{ tag: "projected", path: "/r/./p" }, root], [{ tag: "projected", path: "/r/p/" }, root],
    [{ tag: "projected", path: "/r//p" }, root], [{ tag: "projected", path: "/r/p\0" }, root],
    [{ tag: "projected", path: "/r/\ud800" }, root],
    [{ tag: "projected", path: "/r/p", extra: 1 }, root], [{ tag: "not-outside", path: "/r" }, root],
    [{ tag: "unknown" }, root], [{ tag: "toString" }, root], [{}, root], [null, root],
    [{ tag: "projected", path: "/x" }, "/"], [{ tag: "projected", path: "/r/p" }, "/r/"],
  ];
  for (const [answer, at] of refused) {
    assert.throws(() => encodeProjection(answer, at), OwnedProjectionError, JSON.stringify(answer));
  }
  assert.equal(cleanAbsolute("/"), true);
  for (const path of ["", "r", "/r/", "//", "/r/..", "/r/."]) assert.equal(cleanAbsolute(path), false, path);
});

// --- the three imports ------------------------------------------------------------

function host(answer = () => bytes(2, ...utf8("/r/p"))) {
  const asked = [];
  const projection = createProjectionHost(spelling => {
    asked.push(spelling);
    return typeof answer === "function" ? answer(spelling) : answer;
  });
  const memory = new WebAssembly.Memory({ initial: 1, maximum: 4 });
  const put = (at, values) => new Uint8Array(memory.buffer).set(values, at);
  return { projection, memory, put, asked, imports: projection.imports };
}

test("the imports refuse to run before the checker's memory is bound, and bind once", () => {
  const { projection, memory, imports } = host();
  assert.throws(() => imports.project(0, 1), OwnedBoundaryError);
  assert.throws(() => imports.project_length(), OwnedBoundaryError);
  assert.throws(() => imports.project_copy(0, 0), OwnedBoundaryError);
  projection.bind(memory);
  assert.throws(() => projection.bind(memory), OwnedBoundaryError);
  assert.throws(() => createProjectionHost(() => bytes(0)).bind({ buffer: new ArrayBuffer(8) }), OwnedBoundaryError);
});

test("a spelling is decoded strictly and whole, a leading U+FEFF kept, and a refused one asks nothing", () => {
  const { projection, memory, put, asked, imports } = host();
  projection.bind(memory);
  assert.throws(() => imports.project(0), OwnedBoundaryError);
  assert.throws(() => imports.project(0, 1, 2), OwnedBoundaryError);
  assert.throws(() => imports.project(0.5, 1), OwnedBoundaryError);
  assert.throws(() => imports.project(0, 2 ** 32), OwnedBoundaryError);
  put(0, [0x2f, 0xff]);
  assert.equal(imports.project(0, 2), 1);
  put(0, [0x2f, 0x00, 0x61]);
  assert.equal(imports.project(0, 3), 1);
  assert.equal(imports.project(0, 0), 1);
  assert.equal(imports.project(memory.buffer.byteLength - 1, 2), 21);
  assert.equal(imports.project(-1, 1), 21);
  assert.equal(imports.project(0xffffffff, 0xffffffff), 21);
  assert.deepEqual(asked, []);
  assert.equal(projection.pending(), false);
  put(16, utf8("/\u{feff}t/../x"));
  assert.equal(imports.project(16, utf8("/\u{feff}t/../x").length), 0);
  assert.deepEqual(asked, ["/\u{feff}t/../x"]);
});

test("one answer is held until one exact copy, and nothing else installs, replaces or releases it", () => {
  let next = 0;
  const { projection, memory, put, asked, imports } = host(() => bytes(2, ...utf8(`/r/${next++}`)));
  projection.bind(memory);
  put(0, utf8("/a"));
  assert.equal(imports.project_length(), 0);
  assert.equal(imports.project_copy(100, 0), 1);
  assert.equal(imports.project(0, 2), 0);
  assert.equal(imports.project_length(), 5);
  assert.equal(imports.project_length(), 5);
  // A second question while one answer is held is refused and keeps it.
  assert.equal(imports.project(0, 2), 1);
  assert.equal(imports.project(memory.buffer.byteLength, 2), 1);
  assert.deepEqual(asked, ["/a"]);
  assert.equal(imports.project_copy(100, 4), 1);
  assert.equal(imports.project_copy(memory.buffer.byteLength - 2, 5), 21);
  assert.equal(projection.pending(), true);
  // The held answer is the host's copy: growing the memory changes nothing.
  memory.grow(1);
  assert.equal(imports.project_copy(100, 5), 0);
  assert.deepEqual([...new Uint8Array(memory.buffer, 100, 5)], [2, ...utf8("/r/0")]);
  assert.equal(projection.pending(), false);
  assert.equal(imports.project_length(), 0);
  assert.equal(imports.project_copy(100, 5), 1);
  assert.equal(imports.project(0, 2), 0);
  projection.clear();
  assert.equal(projection.pending(), false);
  assert.equal(imports.project_copy(100, 5), 1);
});

test("an answer that cannot be represented is a terminal failure, never a status", () => {
  const huge = Object.defineProperty(new Uint8Array(1), "length", { value: 2 ** 32 });
  const { projection, memory, put, imports } = host(() => huge);
  projection.bind(memory);
  put(0, utf8("/a"));
  assert.throws(() => imports.project(0, 2), /cannot be represented/);
  assert.equal(projection.pending(), false);
  const empty = host(() => new Uint8Array(0));
  empty.projection.bind(empty.memory);
  empty.put(0, utf8("/a"));
  assert.throws(() => empty.imports.project(0, 2), OwnedBoundaryError);
});

// --- the import inventory ---------------------------------------------------------

/** A module importing `entries` ([module, name, kind]); functions are () -> (). */
function moduleImporting(entries) {
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
  const body = entries.flatMap(([from, name, kind]) => [
    ...leb(from.length), ...text(from), ...leb(name.length), ...text(name),
    ...(kind === "memory" ? [0x02, 0x00, 0x01] : [0x00, 0x00]),
  ]);
  const imports = [...leb(entries.length), ...body];
  return new WebAssembly.Module(new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
    0x02, ...leb(imports.length), ...imports,
  ]));
}

test("each reactor's whole import list must be what the worker serves it", () => {
  const checker = [
    ["wasi_snapshot_preview1", "fd_write"],
    ...["open", "handshake", "exchange", "result_length", "copy_result", "close"].map(name => ["solid_typefacts", name]),
    ...["project", "project_length", "project_copy"].map(name => ["solid_owned_host", name]),
  ];
  checkImportInventory(moduleImporting(checker), "checker", coveredImports);
  checkImportInventory(moduleImporting([["wasi_snapshot_preview1", "fd_write"]]), "producer", coveredImports);
  const refused = [
    ["producer", [["solid_owned_host", "project"]]],
    ["producer", [["solid_typefacts", "open"]]],
    ["checker", [["solid_owned_host", "project_more"]]],
    ["checker", [["solid_owned_host", "toString"]]],
    ["checker", [["elsewhere", "project"]]],
    ["checker", [["wasi_snapshot_preview1", "sock_accept"]]],
    ["checker", [["solid_owned_host", "project", "memory"]]],
  ];
  for (const [role, entries] of refused) {
    assert.throws(() => checkImportInventory(moduleImporting(entries), role, coveredImports), OwnedBoundaryError, JSON.stringify(entries));
  }
});

// --- the store's walk -------------------------------------------------------------

const outsideCalls = (calls, top, root) => calls.filter(([, path]) => typeof path === "string" &&
  path.startsWith(`${top}/`) && path !== root && !path.startsWith(`${root}/`));

test("a spelling that never leaves the namespace is not-outside, with no outside read", () => {
  const { top, root } = tree(project);
  const { store, calls } = open(root);
  assert.deepEqual(store.projectAnswer(`${root}/project/absent/tsconfig.json`), { tag: "not-outside" });
  assert.deepEqual(store.projectAnswer(`${root}/project/tsconfig.json/x`), { tag: "not-outside" });
  assert.deepEqual(store.projectAnswer(`${top}/../${top.split("/").at(-1)}/root/missing`), { tag: "not-outside" });
  assert.deepEqual(outsideCalls(calls, top, root), []);
});

test("outside links into the root project, chains and a virtual-ancestor return included, with exact listings", () => {
  const { top, root } = tree({
    ...project,
    alias: { link: "root/project" },
    hop1: { link: "hop2" }, hop2: { link: `${"root"}/project` },
    toroot: { link: "root" }, "root/up": { link: "../root/project" },
    "root/inner": { link: "project" }, toinner: { link: "root/inner" },
    slashalias: { link: "root/project/" },
  });
  const { store, calls } = open(root);
  const tsconfig = `${root}/project/tsconfig.json`;
  for (const spelling of [
    `${top}/alias/tsconfig.json`, `${top}/hop1/tsconfig.json`, `${top}/toroot/up/tsconfig.json`,
    `${top}/toinner/tsconfig.json`, `${top}/slashalias/tsconfig.json`, `${top}/alias/../project/tsconfig.json`,
    `${top}/alias/./tsconfig.json`,
  ]) {
    assert.deepEqual(store.projectAnswer(spelling), { tag: "projected", path: tsconfig }, spelling);
  }
  assert.deepEqual(store.projectAnswer(`${top}/alias`), { tag: "projected", path: `${root}/project` });
  // Outside reads are lstat and readlink only; listings only inside the root.
  const outside = outsideCalls(calls, top, root);
  assert.ok(outside.length > 0);
  assert.ok(outside.every(([name]) => name === "lstat" || name === "readlink"), JSON.stringify(outside));
  assert.ok(calls.some(([name, path]) => name === "readdir" && path === root));
  assert.ok(calls.some(([name, path]) => name === "readdir" && path === `${root}/project`));
  assert.ok(calls.filter(([name]) => name === "readdir").every(([, path]) => path === root || path.startsWith(`${root}/`)));
  assert.equal(store.fault(), undefined);
  assert.equal(store.validate(), undefined);
});

test("a link read inside the root that leads outside faults, even after an outside link brought the walk in", () => {
  const { top, root } = tree({ ...project, alias: { link: "root" }, "root/escape": { link: "../elsewhere" }, elsewhere: {} });
  const { store } = open(root);
  assert.deepEqual(store.projectAnswer(`${top}/alias/escape/tsconfig.json`), { tag: "faulted" });
  assert.match(store.fault().message, /a link leads outside the directory root/);
  assert.deepEqual(store.projectAnswer(`${top}/alias/project/tsconfig.json`), { tag: "faulted" });
  const ancestor = tree({ ...project, alias: { link: "root" }, "root/up": { link: ".." } });
  const second = open(ancestor.root).store;
  assert.deepEqual(second.projectAnswer(`${ancestor.top}/alias/up/root/project/tsconfig.json`), { tag: "faulted" });
});

test("missing, dangling, not-a-directory, trailing slashes and loops are host errors with their own records", () => {
  const { top, root } = tree({
    ...project, dangling: { link: "nowhere" }, file: "x", loopA: { link: "loopB" }, loopB: { link: "loopA" },
    fileslash: { link: "root/project/tsconfig.json/" }, empty: {},
  });
  const { store } = open(root);
  assert.deepEqual(store.projectAnswer(`${top}/absent/tsconfig.json`), { tag: "host-error", errno: 44 });
  assert.deepEqual(store.projectAnswer(`${top}/dangling/tsconfig.json`), { tag: "host-error", errno: 44 });
  assert.deepEqual(store.observe("R", `${top}/dangling`), { ok: true });
  assert.deepEqual(store.observe("L", `${top}/absent`), { ok: false, code: "ENOENT" });
  assert.equal(store.observe("R", `${top}/absent`), undefined);
  assert.deepEqual(store.projectAnswer(`${top}/file/tsconfig.json`), { tag: "host-error", errno: 54 });
  assert.deepEqual(store.projectAnswer(`${top}/fileslash`), { tag: "host-error", errno: 54 });
  assert.deepEqual(store.projectAnswer(`${top}/file/`), { tag: "host-error", errno: 54 });
  assert.deepEqual(store.projectAnswer(`${top}/loopA/tsconfig.json`), { tag: "host-error", errno: 32 });
  assert.deepEqual(store.projectAnswer(`${top}/empty/../root/project/tsconfig.json`), { tag: "projected", path: `${root}/project/tsconfig.json` });
  assert.equal(store.fault(), undefined);
  assert.equal(store.validate(), undefined);
});

test(`${LINKS} substitutions across outside and inside segments resolve and one more is ELOOP`, () => {
  const layout = { ...project };
  // Outside links o0 -> o1 -> ... -> into the root, then inside links i0 -> ... -> project.
  const build = (outside, inside) => {
    for (let index = 0; index < outside; index++) layout[`o${outside}-${index}`] = { link: index + 1 < outside ? `o${outside}-${index + 1}` : `root/i${outside}-0` };
    for (let index = 0; index < inside; index++) layout[`root/i${outside}-${index}`] = { link: index + 1 < inside ? `i${outside}-${index + 1}` : "project" };
  };
  build(16, LINKS - 16);
  build(17, LINKS - 16);
  const { top, root } = tree(layout);
  const { store } = open(root);
  assert.deepEqual(store.projectAnswer(`${top}/o16-0/tsconfig.json`), { tag: "projected", path: `${root}/project/tsconfig.json` });
  assert.deepEqual(store.projectAnswer(`${top}/o17-0/tsconfig.json`), { tag: "host-error", errno: 32 });
});

test("a path resolving outside the root is outside-root, read by lstat alone", () => {
  const { top, root } = tree({ ...project, "other/tsconfig.json": "{}\n", elsewhere: { link: "other" } });
  const { store, calls } = open(root);
  assert.deepEqual(store.projectAnswer(`${top}/elsewhere/tsconfig.json`), { tag: "outside-root", path: `${top}/other/tsconfig.json`, root });
  assert.ok(outsideCalls(calls, top, root).every(([name]) => name === "lstat" || name === "readlink"));
});

test("injected host errors route: EACCES and EPERM are host errors; a raw ELOOP and a failed readlink fault", () => {
  const { top, root } = tree({ ...project, alias: { link: "root/project" } });
  for (const [code, expected] of [["EACCES", { tag: "host-error", errno: 2 }], ["EPERM", { tag: "host-error", errno: 63 }]]) {
    const { store } = open(root, { lstat: path => { if (path === `${top}/alias`) throw coded(code); return hostSource.lstat(path); } });
    assert.deepEqual(store.projectAnswer(`${top}/alias/tsconfig.json`), expected);
    assert.equal(store.fault(), undefined);
  }
  const loop = open(root, { lstat: path => { if (path === `${top}/alias`) throw coded("ELOOP"); return hostSource.lstat(path); } });
  assert.deepEqual(loop.store.projectAnswer(`${top}/alias/tsconfig.json`), { tag: "faulted" });
  assert.ok(loop.store.fault() instanceof OwnedInputFault);
  const unreadable = open(root, { readlink: path => { if (path === `${top}/alias`) throw coded("EACCES"); return hostSource.readlink(path); } });
  assert.deepEqual(unreadable.store.projectAnswer(`${top}/alias/tsconfig.json`), { tag: "faulted" });
  assert.match(unreadable.store.fault().message, /is a link whose target cannot be read \(EACCES\)/);
});

test("listings are checked root first: a failed one is listing-unavailable, one without the name is name-variant", () => {
  const { top, root } = tree({ ...project, alias: { link: "root/project" } });
  const spelling = `${top}/alias/tsconfig.json`;
  for (const code of ["EACCES", "EPERM", "ENOENT", "ENOTDIR"]) {
    for (const at of [root, `${root}/project`]) {
      const { store } = open(root, { readdir: path => { if (path === at) throw coded(code); return hostSource.readdir(path); } });
      const errno = { EACCES: 2, EPERM: 63, ENOENT: 44, ENOTDIR: 54 }[code];
      assert.deepEqual(store.projectAnswer(spelling), { tag: "listing-unavailable", errno, directory: at });
      assert.deepEqual(store.observe("D", at), { ok: false, code });
      assert.equal(store.fault(), undefined);
    }
  }
  const without = name => path => hostSource.readdir(path).filter(entry => new TextDecoder().decode(entry.name) !== name);
  assert.deepEqual(open(root, { readdir: path => (path === root ? without("project")(path) : hostSource.readdir(path)) }).store.projectAnswer(spelling),
    { tag: "name-variant", path: `${root}/project` });
  assert.deepEqual(open(root, { readdir: path => (path === `${root}/project` ? without("tsconfig.json")(path) : hostSource.readdir(path)) }).store.projectAnswer(spelling),
    { tag: "name-variant", path: `${root}/project/tsconfig.json` });
  // The first problem from the root down decides.
  const rootFails = open(root, { readdir: path => { if (path === root) throw coded("EACCES"); return without("tsconfig.json")(path); } });
  assert.deepEqual(rootFails.store.projectAnswer(spelling), { tag: "listing-unavailable", errno: 2, directory: root });
  const rootLacks = open(root, { readdir: path => { if (path === `${root}/project`) throw coded("EACCES"); return without("project")(path); } });
  assert.deepEqual(rootLacks.store.projectAnswer(spelling), { tag: "name-variant", path: `${root}/project` });
});

test("an unrelated name that is not UTF-8, a listing that changes, or a failed bracketing lstat faults", () => {
  const { top, root } = tree({ ...project, alias: { link: "root/project" } });
  const spelling = `${top}/alias/tsconfig.json`;
  const invalid = open(root, { readdir: path => (path === root ? [...hostSource.readdir(path), { name: new Uint8Array([0xff]), type: "file" }] : hostSource.readdir(path)) });
  assert.deepEqual(invalid.store.projectAnswer(spelling), { tag: "faulted" });
  assert.match(invalid.store.fault().message, /lists a name that is not UTF-8/);
  let listed = false;
  const changing = open(root, {
    readdir: path => { if (path === `${root}/project`) listed = true; return hostSource.readdir(path); },
    lstat: path => {
      const stats = hostSource.lstat(path);
      return path === `${root}/project` && listed ? { ...stats, ino: stats.ino + 1n, isDirectory: () => true, isFile: () => false, isSymbolicLink: () => false } : stats;
    },
  });
  assert.deepEqual(changing.store.projectAnswer(spelling), { tag: "faulted" });
  assert.match(changing.store.fault().message, /changed while it was listed/);
  let readdirs = 0;
  const bracket = open(root, {
    readdir: path => { readdirs++; return hostSource.readdir(path); },
    lstat: path => { if (readdirs > 0 && path === root) throw coded("EIO"); return hostSource.lstat(path); },
  });
  assert.deepEqual(bracket.store.projectAnswer(spelling), { tag: "faulted" });
  assert.match(bracket.store.fault().message, /changed while it was captured/);
  // A sticky fault answers every later question the same way.
  assert.deepEqual(bracket.store.projectAnswer(`${root}/project/tsconfig.json`), { tag: "faulted" });
});

test("projection records are invisible to guests, and ancestors keep their virtual stats and listings", () => {
  const { top, root } = tree({ ...project, alias: { link: "root/project" } });
  const { store } = open(root);
  const before = store.fs.lstatSync(top);
  const listing = store.fs.readdirSync(top, { withFileTypes: true }).map(entry => entry.name);
  assert.deepEqual(store.projectAnswer(`${top}/alias/tsconfig.json`).tag, "projected");
  assert.deepEqual(store.observe("L", `${top}/alias`), { ok: true });
  for (const call of [() => store.fs.lstatSync(`${top}/alias`), () => store.fs.readlinkSync(`${top}/alias`),
    () => store.fs.statSync(`${top}/alias/tsconfig.json`), () => store.fs.openSync(`${top}/alias/tsconfig.json`, 0),
    () => store.fs.readdirSync(`${top}/alias`, { withFileTypes: true })]) {
    assert.throws(call, error => error?.code === "ENOENT");
  }
  const after = store.fs.lstatSync(top);
  assert.equal(identityOf(after), identityOf(before));
  assert.equal(after.isDirectory(), true);
  assert.equal(after.dev, 0n);
  assert.deepEqual(store.fs.readdirSync(top, { withFileTypes: true }).map(entry => entry.name), listing);
  assert.deepEqual(listing, ["root"]);
  assert.equal(store.fault(), undefined);
});

test("validation re-performs projection records, successes and errors, and a later question reuses them", () => {
  const { top, root } = tree({ ...project, alias: { link: "root/project" }, other: {} });
  const swapped = open(root);
  assert.equal(swapped.store.projectAnswer(`${top}/alias/tsconfig.json`).tag, "projected");
  assert.equal(swapped.store.validate(), undefined);
  const counted = swapped.store.metrics().records;
  assert.equal(swapped.store.projectAnswer(`${top}/alias/tsconfig.json`).tag, "projected");
  assert.equal(swapped.store.metrics().records, counted);
  nodeFs.unlinkSync(join(top, "alias"));
  nodeFs.symlinkSync("other", join(top, "alias"));
  assert.match(swapped.store.validate().message, /changed since it was read/);
  const missing = open(root);
  assert.deepEqual(missing.store.projectAnswer(`${top}/later/tsconfig.json`), { tag: "host-error", errno: 44 });
  nodeFs.symlinkSync("root/project", join(top, "later"));
  assert.match(missing.store.validate().message, /changed since it was read/);
});

// --- real host observations (each engine runs this file) --------------------------

test("on this host: a search-only parent inside the root is a reached listing denial on an existing path", () => {
  const { top, root } = tree({ "root/sub/proj/tsconfig.json": "{}\n", subalias: { link: "root/sub/proj" } });
  nodeFs.chmodSync(join(root, "sub"), 0o111);
  // The scenario is real on this engine, or the test fails rather than passing vacuously.
  assert.ok(nodeFs.lstatSync(join(root, "sub/proj/tsconfig.json")).isFile());
  assert.throws(() => nodeFs.readdirSync(join(root, "sub")), error => error?.code === "EACCES");
  const { store } = open(root);
  assert.deepEqual(store.projectAnswer(`${top}/subalias/tsconfig.json`), { tag: "listing-unavailable", errno: 2, directory: `${root}/sub` });
  assert.deepEqual(store.observe("D", `${root}/sub`), { ok: false, code: "EACCES" });
  assert.equal(store.validate(), undefined);
  nodeFs.chmodSync(join(root, "sub"), 0o755);
  assert.ok(nodeFs.lstatSync(join(root, "sub/proj/tsconfig.json")).isFile());
});

test("on this host: an outside directory without search permission is a reached lookup denial on an existing path", () => {
  const { top, root } = tree({ ...project, "locked/inner": { link: "../root/project" } });
  nodeFs.chmodSync(join(top, "locked"), 0o600);
  assert.throws(() => nodeFs.lstatSync(join(top, "locked/inner")), error => error?.code === "EACCES");
  const { store } = open(root);
  assert.deepEqual(store.projectAnswer(`${top}/locked/inner/tsconfig.json`), { tag: "host-error", errno: 2 });
  assert.deepEqual(store.observe("L", `${top}/locked/inner`), { ok: false, code: "EACCES" });
  nodeFs.chmodSync(join(top, "locked"), 0o755);
  assert.ok(nodeFs.lstatSync(join(top, "locked/inner")).isSymbolicLink());
});

test("on this host: a case-variant link target is name-variant when the volume ignores case, absent otherwise", () => {
  const { top, root } = tree({ ...project, variant: { link: "root/PROJECT" } });
  const { store } = open(root);
  let insensitive = true;
  try {
    nodeFs.lstatSync(join(root, "PROJECT"));
  } catch {
    insensitive = false;
  }
  assert.deepEqual(store.projectAnswer(`${top}/variant/tsconfig.json`),
    insensitive ? { tag: "name-variant", path: `${root}/PROJECT` } : { tag: "host-error", errno: 44 });
});
