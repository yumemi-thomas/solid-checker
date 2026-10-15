// The mount is captured once: a getter or a mutation during capture cannot
// change what was validated, and the copy is the package's own.
import assert from "node:assert/strict";
import { test } from "vitest";
import { OwnedMountError, validateMount } from "../owned-mount.mjs";

const text = bytes => new TextDecoder().decode(bytes);
const shape = mount => mount.entries.map(entry => [entry.kind, entry.path, entry.target ?? (entry.bytes && text(entry.bytes))]);

test("a path getter is read once, and its later answer never enters the mount", () => {
  let reads = 0;
  const mount = validateMount({
    root: "/m",
    entries: [{ kind: "file", get path() { return ++reads === 1 ? "/m/a" : "/outside"; }, bytes: "x" }],
  });
  assert.equal(reads, 1);
  assert.deepEqual(shape(mount), [["file", "/m/a", "x"]]);
});

test("a link target getter is read once", () => {
  let reads = 0;
  const mount = validateMount({
    root: "/m",
    entries: [{ kind: "symlink", path: "/m/link", get target() { return ++reads === 1 ? "a" : "/outside"; } }],
  });
  assert.equal(reads, 1);
  assert.deepEqual(shape(mount), [["symlink", "/m/link", "a"]]);
});

test("a getter whose first answer is invalid is refused on that answer", () => {
  let reads = 0;
  assert.throws(() => validateMount({
    root: "/m",
    entries: [{ kind: "file", get path() { return ++reads === 1 ? "/outside" : "/m/a"; }, bytes: "x" }],
  }), error => error instanceof OwnedMountError && error.message === "mount entry /outside is not under the mount root /m");
  assert.equal(reads, 1);
});

test("mutation during capture changes nothing already captured or not yet listed", () => {
  const bytes = new TextEncoder().encode("original");
  const entries = [
    { kind: "file", path: "/m/a", bytes: "a" },
    { kind: "file", path: "/m/b", bytes },
  ];
  entries.push({
    kind: "file",
    get path() {
      entries[0].path = "/outside/a";
      bytes.fill(0x21);
      entries.push({ kind: "file", path: "/outside/extra", bytes: "" });
      return "/m/c";
    },
    bytes: "c",
  });
  const mount = validateMount({ root: "/m", entries });
  assert.deepEqual(shape(mount), [["file", "/m/a", "a"], ["file", "/m/b", "original"], ["file", "/m/c", "c"]]);
});

test("file bytes are copied: later writes to the caller's buffer change nothing", () => {
  const bytes = new TextEncoder().encode("before");
  const mount = validateMount({ root: "/m", entries: [{ kind: "file", path: "/m/a", bytes }] });
  bytes.fill(0x21);
  assert.equal(text(mount.entries[0].bytes), "before");
  assert.notEqual(mount.entries[0].bytes.buffer, bytes.buffer);
  assert.ok(Object.isFrozen(mount) && Object.isFrozen(mount.entries) && Object.isFrozen(mount.entries[0]));
});
