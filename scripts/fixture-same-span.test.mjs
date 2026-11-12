// The fixtures whose claim needs two files' calls at one byte offset with one length (ADR 0961 amended, batch 11 slice 9A): the after-await
// walk's `visited` set was keyed by span alone, so a call in a second file at the offsets of one the walk had entered in the first was skipped.
// Each call is bracketed by `/*[form*/ ... /*]form*/`; each `<form>-b.ts` pads its call to the offset of the one in App.tsx. An edit above a
// call moves it, and the fixture would then test nothing (the collision gone, the row a violation either way) with every gate green.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const root = new URL("../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");
const FIXTURES = ["fixtures/reactive-ir/after-await-cross-file-same-span", "fixtures/runtime-oracle/after-await-cross-file-same-span"];
const FORMS = ["helper", "iife", "tag", "each", "none"];
const FILES = ["App.tsx", "state.ts", ...FORMS.map((form) => `${form}-b.ts`)];

/** The offset of the first byte after the form's open marker, and the length up to its close marker. */
function markedSpan(text, form) {
  const open = `/*[${form}*/`;
  const close = `/*]${form}*/`;
  const start = text.indexOf(open);
  assert.ok(start >= 0 && text.indexOf(open, start + 1) < 0, `${form}: exactly one open marker`);
  const end = text.indexOf(close);
  assert.ok(end > start, `${form}: a close marker after the open one`);
  return { offset: start + open.length, length: end - (start + open.length) };
}

for (const fixture of FIXTURES) {
  for (const form of FORMS) {
    test(`${fixture}: the ${form} calls of App.tsx and ${form}-b.ts stand at one offset with one length`, () => {
      const app = markedSpan(read(`${fixture}/App.tsx`), form);
      const other = markedSpan(read(`${fixture}/${form}-b.ts`), form);
      assert.deepEqual(other, app);
    });
  }
}

test("the runtime-oracle case holds the fixture's files byte for byte", () => {
  for (const file of FILES) assert.equal(read(`${FIXTURES[1]}/${file}`), read(`${FIXTURES[0]}/${file}`), file);
});

test("markedSpan reads offsets and lengths", () => {
  assert.deepEqual(markedSpan("ab/*[x*/foo()/*]x*/;", "x"), { offset: 8, length: 5 });
  assert.throws(() => markedSpan("no markers", "x"));
});
