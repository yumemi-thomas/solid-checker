import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "vitest";

import { ALLOWLIST, PENDING, acknowledges, countReads, measure, readSites, readSitesWithText, replacedSites, risesOf, violations } from "./ir-read-lint.mjs";

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
        let _ = lookup.file_slots().get(3);
        let _ = lookup.file_by_path("x");
        let _ = self.symbols_by_id().get("x");
        let seen: HashSet<String> = HashSet::default();
        let by_name = HashMap::<String, u8>::default();
        for name in &seen {}
        let _ = by_name.keys();
        let _ = by_name.get("x");
        let _ = facts.typescript.entities().count();
        let _ = self.facts.typescript.symbols();
        let _ = facts.typescript.entities_for_path("x");
        let _ = self.symbol_names.values();
        let _ = self.symbol_names.get("x");
        let _ = lookup.placing_callee_call(file, callee);
    }
    #[cfg(test)]
    mod tests {
        fn b(facts: &ProjectFacts) { for file in facts.files.iter() {} }
    }
  `);
  assert.deepEqual(counts, {
    "facts.files": 1,
    file_slots: 1,
    file_by_path: 1,
    symbols_by_id: 1,
    "typescript-table": 2,
    "placing-call": 1,
    "symbol_names-scan": 1,
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

test("production code after a #[cfg(test)] item is scanned; the item itself is not", () => {
  const text = [
    "#[cfg(test)]",
    "fn helper(facts: &ProjectFacts) {",
    '    let brace = "}";',
    "    for file in facts.files.iter() {}",
    "}",
    "struct S {",
    "    #[cfg(test)]",
    "    seen: Vec<u8>,",
    "}",
    "#[cfg(test)]",
    "use crate::x;",
    "fn production(lookup: &SemanticLookup) {",
    '    let _ = lookup.file_by_path("x");',
    "}",
    "#[cfg(test)]",
    "mod tests {",
    "    fn b(facts: &ProjectFacts) { for file in facts.files.iter() {} }",
    "}",
    "fn after(facts: &ProjectFacts) { let _ = facts.files.len(); }",
    ""
  ].join("\n");
  assert.deepEqual(readSites(text), [
    { line: 13, kind: "file_by_path" },
    { line: 19, kind: "facts.files" }
  ]);
});

test("a read rustfmt splits over lines is counted at the line it starts on", () => {
  const text = ["fn a(facts: &ProjectFacts) {", "    let n = facts", "        .files", "        .len();", "}", ""].join("\n");
  assert.deepEqual(readSites(text), [{ line: 2, kind: "facts.files" }]);
});

test("a pending-review list is carried, not counted", () => {
  const allowed = { "a.rs": { file_by_path: 1, [PENDING]: ["1 file_by_path: why"] } };
  assert.deepEqual(violations(allowed, { "a.rs": { file_by_path: 1 } }), []);
});

// ADR 0995: a count that holds can hide a fixed site and a new one.
const site = (kind, text) => ({ kind, text });

test("a fixed read and a new one in the same file and kind keep the count and are a replaced site", () => {
  // The laundering: the recorded `file_by_path` read at the top of the file is made through a query, and another raw read is added below:
  // 1 allowed, 1 found, and the lint is green over a read the reuse logic cannot see.
  const base = { "a.rs": [site("file_by_path", "let f = lookup.file_by_path(path);")] };
  const head = { "a.rs": [site("file_by_path", "let g = lookup.file_by_path(other);")] };
  assert.deepEqual(violations({ "a.rs": { file_by_path: 1 } }, { "a.rs": { file_by_path: 1 } }), []);
  assert.deepEqual(replacedSites(base, head), [{ path: "a.rs", kind: "file_by_path", text: "let g = lookup.file_by_path(other);" }]);
  // The same text (a reflow normalizes to it), another file's or another kind's does not pair; an added site is a rise, not a replacement.
  assert.deepEqual(replacedSites(base, { "a.rs": [site("file_by_path", "let f = lookup.file_by_path(path);")] }), []);
  assert.deepEqual(replacedSites(base, { "a.rs": [...base["a.rs"], site("file_by_path", "new();")] }), []);
  assert.deepEqual(replacedSites(base, { "b.rs": [site("file_by_path", "x();")] }), []);
  assert.deepEqual(replacedSites({}, { "b.rs": [site("file_by_path", "x();")] }), []);
});

test("a replaced site is acknowledged only by its text named in the entry's pending-review list", () => {
  const entry = { file_by_path: 1, [PENDING]: ["file_by_path site replaced (--allow-rise): unreviewed `let g = lookup.file_by_path(other);`"] };
  assert.equal(acknowledges(entry, "let g = lookup.file_by_path(other);"), true);
  assert.equal(acknowledges(entry, "let h = lookup.file_by_path(third);"), false);
  assert.equal(acknowledges({ file_by_path: 1 }, "let g = lookup.file_by_path(other);"), false);
});

test("a rise is found against the recorded counts, including a new file or kind", () => {
  const allowed = { "a.rs": { file_by_path: 1, [PENDING]: ["x"] } };
  assert.deepEqual(risesOf(allowed, { "a.rs": { file_by_path: 1 } }), []);
  assert.deepEqual(risesOf(allowed, { "a.rs": { file_by_path: 2, file_slots: 1 }, "b.rs": { "facts.files": 1 } }), [
    { path: "a.rs", kind: "file_by_path", was: 1, now: 2 },
    { path: "a.rs", kind: "file_slots", was: 0, now: 1 },
    { path: "b.rs", kind: "facts.files", was: 0, now: 1 },
  ]);
  assert.deepEqual(risesOf(allowed, { "a.rs": { file_by_path: 0 } }), []);
});

test("sites carry their normalized line text", () => {
  const sites = readSitesWithText("fn a() {\n    let   f = lookup.file_by_path(p);\n}\n");
  assert.deepEqual(sites, [{ line: 2, kind: "file_by_path", text: "let f = lookup.file_by_path(p);" }]);
});
