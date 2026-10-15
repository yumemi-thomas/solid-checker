// The owned check's lifecycle from the built package, without a native
// reference: an answer, a refusal that leaves the checker usable, reuse,
// close, and refused mounts. Native parity is `make wasm-parity`'s, not this
// file's. The artifacts must be built (`bun run build`); a missing one fails.
import assert from "node:assert/strict";
import * as nodeFs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";
import { createOwnedChecker, mountDirectory, OwnedMountError } from "../owned.mjs";

const root = "/workspace/owned-smoke";
const mount = {
  root,
  entries: [
    { kind: "file", path: `${root}/tsconfig.json`, bytes: '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n' },
    { kind: "file", path: `${root}/App.ts`, bytes: "export const answer = 42;\n" },
  ],
};

test("an owned checker answers, refuses, is reused and closes", async () => {
  const checker = await createOwnedChecker({ mount });
  const first = await checker.check({ projectId: `${root}/tsconfig.json` });
  assert.equal(first.ok, true, first.error);
  assert.equal(typeof first.snapshot.status, "string");
  assert.deepEqual(first.transport.opened, first.transport.closed);
  assert.ok(first.transport.exchanges > 0);
  assert.deepEqual(first.transport.pendingResults, []);

  const refusal = await checker.check({ projectId: `${root}/absent/tsconfig.json` });
  assert.equal(refusal.ok, false);
  assert.equal(refusal.error, "No such file or directory (os error 44)");
  assert.deepEqual(refusal.transport.calls, []);

  const again = await checker.check({ projectId: `${root}/tsconfig.json` });
  assert.equal(again.json, first.json);

  await checker.close();
  await assert.rejects(checker.check({ projectId: `${root}/tsconfig.json` }), /the owned checker is closed/);
});

test("a mount is validated before anything is copied", async () => {
  await assert.rejects(
    createOwnedChecker({ mount: { root, entries: [{ kind: "symlink", path: `${root}/escape`, target: "../outside" }] } }),
    error => error instanceof OwnedMountError && error.message === `mount link ${root}/escape -> ../outside leaves the mount root ${root}`,
  );
  await assert.rejects(
    createOwnedChecker({ mount: { root, entries: [{ kind: "file", path: `${root}/a/b`, bytes: "" }] } }),
    error => error instanceof OwnedMountError && error.message === `mount entry ${root}/a/b has no declared parent directory ${root}/a`,
  );
  await assert.rejects(createOwnedChecker({ mount, adapters: {} }), /createOwnedChecker has no option "adapters"/);
});

// Directory mode (P5 S3b): the same project on disk, read on demand.
function project(files) {
  const top = nodeFs.realpathSync(nodeFs.mkdtempSync(join(tmpdir(), "owned-directory-")));
  const root = join(top, "project");
  nodeFs.mkdirSync(root);
  for (const [path, text] of Object.entries(files)) nodeFs.writeFileSync(join(root, path), text);
  return { top, root, remove: () => nodeFs.rmSync(top, { recursive: true, force: true }) };
}
const smokeFiles = {
  "tsconfig.json": '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n',
  "App.ts": "export const answer = 42;\n",
};

test("a directory checker answers exactly as a mount of the same tree, refuses, and is reused", async () => {
  const { root, remove } = project(smokeFiles);
  try {
    const viaMount = await createOwnedChecker({ mount: mountDirectory(root) });
    const expected = await viaMount.check({ projectId: `${root}/tsconfig.json` });
    await viaMount.close();
    const checker = await createOwnedChecker({ directory: root });
    const first = await checker.check({ projectId: `${root}/tsconfig.json` });
    assert.equal(first.ok, true, first.error);
    assert.equal(first.json, expected.json);
    assert.deepEqual(first.transport, expected.transport);
    const refusal = await checker.check({ projectId: `${root}/absent/tsconfig.json` });
    assert.equal(refusal.ok, false);
    assert.equal(refusal.error, "No such file or directory (os error 44)");
    const again = await checker.check({ projectId: `${root}/tsconfig.json` });
    assert.equal(again.json, first.json);
    await checker.close();
  } finally {
    remove();
  }
});

test("input that changes after it was read rejects that check, the queued ones and every later one, with the store's reason", async () => {
  const { root, remove } = project(smokeFiles);
  try {
    const checker = await createOwnedChecker({ directory: root });
    assert.equal((await checker.check({ projectId: `${root}/tsconfig.json` })).ok, true);
    nodeFs.writeFileSync(join(root, "App.ts"), "export const answer = 43;\n");
    const active = checker.check({ projectId: `${root}/tsconfig.json` });
    const queued = checker.check({ projectId: `${root}/tsconfig.json` });
    const reason = /^the owned checker is poisoned: the owned checker's input faulted: .*App\.ts: changed since it was read/;
    const error = await active.then(() => assert.fail("answered"), failure => failure);
    assert.match(error.message, reason);
    await assert.rejects(queued, failure => failure === error);
    await assert.rejects(checker.check({ projectId: `${root}/tsconfig.json` }), failure => failure === error);
    await checker.close();
  } finally {
    remove();
  }
});

test("a check that reaches a link leaving the root is rejected with that reason, never answered", async () => {
  const { top, root, remove } = project({
    "tsconfig.json": '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts", "escape/Out.ts"] }\n',
    "App.ts": "export const answer = 42;\n",
  });
  try {
    nodeFs.mkdirSync(join(top, "outside"));
    nodeFs.writeFileSync(join(top, "outside/Out.ts"), "export const out = 1;\n");
    nodeFs.symlinkSync("../outside", join(root, "escape"));
    const checker = await createOwnedChecker({ directory: root });
    await assert.rejects(checker.check({ projectId: `${root}/tsconfig.json` }),
      /^OwnedCheckerError: the owned checker is poisoned: the owned checker's input faulted: .*a link leads outside the directory root/);
    await checker.close();
  } finally {
    remove();
  }
});

test("options are captured once: one key, read once, and directory must name a directory", async () => {
  const { root, remove } = project(smokeFiles);
  try {
    let reads = 0;
    const options = { get directory() { reads++; return reads === 1 ? root : "/"; } };
    const checker = await createOwnedChecker(options);
    assert.equal(reads, 1);
    assert.equal((await checker.check({ projectId: `${root}/tsconfig.json` })).ok, true);
    await checker.close();
    await assert.rejects(createOwnedChecker({ directory: root, mount }), /exactly one of mount or directory/);
    await assert.rejects(createOwnedChecker({ directory: root, [Symbol("x")]: 1 }), /has no option Symbol\(x\)/);
    await assert.rejects(createOwnedChecker({ directory: 7 }), TypeError);
    await assert.rejects(createOwnedChecker({ directory: join(root, "App.ts") }), OwnedMountError);
    await assert.rejects(createOwnedChecker({ directory: "/" }), OwnedMountError);
    await assert.rejects(createOwnedChecker({}), error => error instanceof OwnedMountError && /mount must be an object/.test(error.message));
  } finally {
    remove();
  }
});
