import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

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
