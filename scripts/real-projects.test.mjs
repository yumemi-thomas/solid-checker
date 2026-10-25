// The real-project edit benchmark's pure parts (ADR 0423): manifest
// validation, the edits, and the paired ratio gate.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "vitest";

import { ceilingFor, editText, manifestErrors, markdownTable, median, signP, summarize } from "../benchmarks/lib/real-projects.mjs";

const manifest = JSON.parse(readFileSync("benchmarks/real-projects/manifest.json", "utf8"));

test("the committed manifest is valid and every patch it names exists", () => {
  assert.deepEqual(manifestErrors(manifest), []);
  for (const project of manifest.projects) {
    assert.ok(existsSync(`benchmarks/real-projects/${project.patch}`), project.patch);
  }
  assert.deepEqual(manifest.projects.map(project => project.name), ["app-game", "sensor-sim", "solid-groove"]);
});

test("manifest validation names each defect", () => {
  const good = structuredClone(manifest.projects[0]);
  const errorsFor = change => manifestErrors({ format: 1, projects: [{ ...good, ...change }] });
  assert.deepEqual(errorsFor({}), []);
  assert.match(errorsFor({ commit: "3064fed" }).join(), /40-hex/);
  assert.match(errorsFor({ lockfileDigest: "abc" }).join(), /sha256/);
  assert.match(errorsFor({ repo: "git@github.com:x/y" }).join(), /https/);
  assert.match(errorsFor({ tsconfig: "../outside.json" }).join(), /relative path/);
  assert.match(errorsFor({ kinds: ["code", "rename"] }).join(), /unknown edit kind rename/);
  assert.match(errorsFor({ kinds: ["code", "code"] }).join(), /duplicate edit kind/);
  assert.match(errorsFor({ bodyAnchor: undefined }).join(), /needs bodyAnchor/);
  assert.match(errorsFor({ kinds: ["code"] }).join(), /bodyAnchor without/);
  assert.match(manifestErrors({ format: 1, projects: [good, good] }).join(), /duplicate name/);
  assert.match(manifestErrors({ format: 2, projects: [] }).join(), /format/);
});

test("each edit adds one distinct declaration where its kind says", () => {
  const source = "import a from 'a';\nimport b from 'b';\n\nconst x = 1;\nconst y = 2;\n\nfunction AppTitle() {\n  return 1;\n}\n";
  assert.equal(editText(source, "code", 7), `${source}export const __q7 = 7;\n`);
  assert.equal(editText(source, "import", 7), `import { createSignal as __s7 } from "solid-js";\n${source}`);
  assert.match(editText(source, "mid", 7), /const y = 2;\nexport const __m7 = 7;\n\nfunction/);
  assert.match(editText(source, "body", 7, "function AppTitle() {"), /function AppTitle\(\) \{\n {2}const __b7 = 7;\n {2}return 1;/);
  assert.throws(() => editText(source, "body", 7, "function Missing() {"), /exactly one line/);
  assert.throws(() => editText("a\nb\n", "mid", 7), /no blank line/);
  const crlf = source.replaceAll("\n", "\r\n");
  assert.equal(editText(crlf, "code", 7), `${crlf}export const __q7 = 7;\r\n`);
  assert.match(editText(crlf, "body", 7, "function AppTitle() {"), /\{\r\n {2}const __b7 = 7;\r\n/);
});

test("median and the two-sided sign test", () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 2, 3]), 2.5);
  assert.equal(signP(0, 0), 1);
  assert.equal(signP(5, 0), 2 / 32);
  assert.equal(signP(3, 2), 1);
});

const row = (project, kind, rep, arm, ms, extra = {}) => ({ project, kind, rep, arm, ms, hash: "h", retained: true, ...extra });
const pairs = (project, kind, base, head, extra = {}) =>
  base.flatMap((ms, rep) => [row(project, kind, rep, "base", ms, extra.base), row(project, kind, rep, "head", head[rep], extra.head)]);

test("the gate is the median of paired ratios, not the ratio of medians", () => {
  // Medians 100 and 120 (1.2), but the paired ratios are 1.3, 1.3, 1.0.
  const [line] = summarize(pairs("p", "code", [100, 90, 200], [130, 117, 120]));
  assert.equal(line.ratio, 1.3);
  assert.equal(line.fails, true);
  assert.match(line.notes.join(), /ratio above 1.25/);
  const [ok] = summarize(pairs("p", "code", [100, 100, 100], [125, 125, 125]));
  assert.equal(ok.ratio, 1.25);
  assert.equal(ok.fails, false);
});

test("differing answers are reported and leave the ratio ungated", () => {
  const rows = pairs("p", "code", [100, 100], [200, 200], { head: { hash: "other" } });
  const [line] = summarize(rows);
  assert.equal(line.answersEqual, false);
  assert.equal(line.ratioGated, false);
  assert.equal(line.fails, false);
  assert.match(markdownTable([line]), /\| NO \|.*answers differ/);
});

test("a head answer outside its retained daemon fails; a base one only ungates", () => {
  const [head] = summarize(pairs("p", "code", [100], [50], { head: { retained: false } }));
  assert.equal(head.fails, true);
  const [base] = summarize(pairs("p", "code", [100], [500], { base: { retained: false } }));
  assert.equal(base.fails, false);
  assert.equal(base.ratioGated, false);
});

test("ceilings report until enforced", () => {
  assert.equal(ceilingFor("app-game", "code"), 1000);
  assert.equal(ceilingFor("app-game", "import"), 2500);
  const rows = pairs("app-game", "code", [1100], [1100]);
  const [reported] = summarize(rows);
  assert.equal(reported.fails, false);
  assert.match(reported.notes.join(), /above 1000 ms \(reported\)/);
  const [enforced] = summarize(rows, { enforceCeilings: true });
  assert.equal(enforced.fails, true);
});

test("an incomplete pair is not a measurement", () => {
  const [line] = summarize([row("p", "code", 0, "base", 100)]);
  assert.equal(line.n, 0);
  assert.equal(line.fails, true);
});
