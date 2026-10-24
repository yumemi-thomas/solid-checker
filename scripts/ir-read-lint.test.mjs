import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "vitest";

import { ALLOWLIST, countReads, measure, violations } from "./ir-read-lint.mjs";

test("the IR has no raw cross-file read beyond the allow-list", () => {
  assert.ok(existsSync(ALLOWLIST));
  assert.deepEqual(violations(JSON.parse(readFileSync(ALLOWLIST, "utf8")), measure()), []);
});

test("each raw read kind is counted, comments and test modules are not", () => {
  const counts = countReads(`
    // facts.files in prose, file_by_path in prose
    fn a(lookup: &SemanticLookup, facts: &ProjectFacts) {
        for file in facts.files.iter() {}
        for file in lookup.files() {}
        let _ = lookup.file_by_path("x");
        let seen: HashSet<String> = HashSet::default();
        let by_name = HashMap::<String, u8>::default();
        for name in &seen {}
        let _ = by_name.keys();
        let _ = by_name.get("x");
    }
    #[cfg(test)]
    mod tests {
        fn b(facts: &ProjectFacts) { for file in facts.files.iter() {} }
    }
  `);
  assert.deepEqual(counts, {
    "facts.files": 1,
    "files()": 1,
    file_by_path: 1,
    "hashmap-iteration": 2
  });
});

test("a new site fails and a removed site asks for the allow-list to shrink", () => {
  const allowed = { "a.rs": { "facts.files": 2 } };
  assert.equal(violations(allowed, { "a.rs": { "facts.files": 2 } }).length, 0);
  assert.match(violations(allowed, { "a.rs": { "facts.files": 3 } })[0], /new raw cross-file read/);
  assert.match(violations(allowed, { "a.rs": { "facts.files": 1 } })[0], /--update/);
  assert.match(violations(allowed, { "a.rs": { "facts.files": 2 }, "b.rs": { file_by_path: 1 } })[0], /b\.rs/);
});
