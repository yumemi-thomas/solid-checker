// The owned check's lifecycle from the built package, without a native
// reference: an answer, a refusal that leaves the checker usable, reuse,
// close, and refused mounts. Native parity is `make wasm-parity`'s, not this
// file's. The artifacts must be built (`bun run build`); a missing one fails.
import assert from "node:assert/strict";
import { test } from "vitest";
import { createOwnedChecker, OwnedMountError } from "../owned.mjs";

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
