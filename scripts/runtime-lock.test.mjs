import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "vitest";

const root = resolveRoot();

function resolveRoot() {
  return join(import.meta.dirname, "..");
}

test("runtime lock pins the Solid 2 transitive runtime edge", () => {
  const lock = JSON.parse(
    readFileSync(join(root, "pkg/contracts/bundled/runtime-lock.json"), "utf8")
  );
  assert.equal(lock.schemaVersion, 2);
  assert.equal(lock.format, "solid-checker-package-runtime-lock");
  const edge = lock.packages["solid-js@2.0.0-rc.3"].dependencies["@solidjs/signals"];
  assert.deepEqual(edge, {
    range: "^2.0.0-rc.3",
    version: "2.0.0-rc.3",
    integrity:
      "sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA=="
  });
  assert.equal(
    lock.packages["@solidjs/web@2.0.0-rc.3"].peerDependencies["solid-js"].version,
    "2.0.0-rc.3"
  );
});

test("runtime lock pins the Solid 1.x probe closure", () => {
  const lock = JSON.parse(
    readFileSync(join(root, "pkg/contracts/bundled/runtime-lock.json"), "utf8")
  );
  // The scheduled overlay is probed against solid-js 1.x, which resolves its own
  // transitive tree. Without these edges the 1.x probes would run against
  // whatever the package manager picked for csstype/seroval on the day they ran.
  const solid1 = lock.packages["solid-js@1.9.14"];
  assert.deepEqual(Object.keys(solid1.dependencies).sort(), [
    "csstype",
    "seroval",
    "seroval-plugins"
  ]);
  // The peer is pinned to the exact release the dialect audits, not to the
  // range the package declares.
  const scheduled = lock.packages["@solid-primitives/scheduled@1.5.3"];
  assert.equal(scheduled.peerDependencies["solid-js"].range, "^1.6.12");
  assert.equal(scheduled.peerDependencies["solid-js"].version, "1.9.14");
  const debounce = lock.packages["@solid-primitives/debounce@1.3.0"];
  assert.equal(debounce.peerDependencies["solid-js"].range, ">=1.0.0");
  assert.equal(debounce.peerDependencies["solid-js"].version, "1.9.14");
  const rootless = lock.packages["@solid-primitives/rootless@1.5.4"];
  assert.equal(rootless.peerDependencies["solid-js"].version, "1.9.14");
});

test("receipt-bound contract documents are checked out with LF bytes", () => {
  const documents = [
    "pkg/contracts/bundled/solid-v2/solid-js.json",
    "rust/crates/solid-dialect/contracts/solid-v2/solid-js.json",
    "fixtures/reactive-ir/package-consumer/node_modules/reactive-package/solid-reactivity.json"
  ];
  const result = spawnSync("git", ["check-attr", "eol", "--", ...documents], {
    cwd: root,
    encoding: "utf8"
  });

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(
    result.stdout.trim().split("\n"),
    documents.map(document => `${document}: eol: lf`)
  );
});
