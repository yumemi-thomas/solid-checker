import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { contextSpec, scenarioCounts } from "./oracle-context.mjs";

test("absent context fields are null and change nothing", () => {
  assert.deepEqual(contextSpec("c", {}), { scenario: null, requires: null, reaches: null });
});

test("valid context fields are carried as written", () => {
  const spec = { scenario: "api-example", requires: ["an Errored boundary"], reaches: "assert-text step `shown`" };
  assert.deepEqual(contextSpec("c", spec), spec);
});

test("a malformed context field refuses the case", () => {
  assert.throws(() => contextSpec("c", { scenario: "real-world" }), /c: scenario is one of/);
  assert.throws(() => contextSpec("c", { requires: "props" }), /c: requires is a non-empty list/);
  assert.throws(() => contextSpec("c", { requires: [] }), /requires/);
  assert.throws(() => contextSpec("c", { requires: ["ok", " "] }), /requires/);
  assert.throws(() => contextSpec("c", { reaches: 3 }), /c: reaches is a non-empty string/);
  assert.throws(() => contextSpec("c", { reaches: "" }), /reaches/);
});

test("scenarioCounts counts per kind, with unspecified for none", () => {
  assert.deepEqual(
    scenarioCounts([{ scenario: "application" }, { scenario: "application" }, { scenario: null }, {}, { scenario: "constructed-misuse" }]),
    { application: 2, unspecified: 2, "constructed-misuse": 1 },
  );
});

test("every committed oracle.json passes the context validation", () => {
  const root = join(dirname(fileURLToPath(import.meta.url)), "../fixtures/runtime-oracle");
  for (const name of readdirSync(root)) {
    let text;
    try { text = readFileSync(join(root, name, "oracle.json"), "utf8"); } catch { continue; }
    contextSpec(name, JSON.parse(text));
  }
});
