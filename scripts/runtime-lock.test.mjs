import assert from "node:assert/strict";
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
  const edge = lock.packages["solid-js@2.0.0-rc.0"].dependencies["@solidjs/signals"];
  assert.deepEqual(edge, {
    range: "^2.0.0-rc.0",
    version: "2.0.0-rc.0",
    integrity:
      "sha512-oKZSfvsCcKw1uJjOGbUkJ+OqlhXLHtZ+rShSyu9KH0lUH7UUwfMfsKeh81JPiQxDDg4YLhEwI38hg0JkwzTdvA=="
  });
  assert.equal(
    lock.packages["@solidjs/web@2.0.0-rc.0"].peerDependencies["solid-js"].version,
    "2.0.0-rc.0"
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
});
