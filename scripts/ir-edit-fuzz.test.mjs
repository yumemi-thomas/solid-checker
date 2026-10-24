import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { EDIT_KINDS, FIXTURE_PROJECTS, ROOT, Restorer, restoreJournal, seededRandom } from "./ir-edit-fuzz.mjs";
import { existsSync } from "node:fs";

const SAMPLE = `import { createSignal } from "solid-js";
import { helper } from "./helper";

export const [count, setCount] = createSignal(10);
export function Widget() {
  return <div>{count()} {"label"}</div>;
}
`;

test("the same seed replays the same sequence", () => {
  const first = seededRandom(7);
  const second = seededRandom(7);
  const draw = (rand) => Array.from({ length: 8 }, () => rand(1000));
  assert.deepEqual(draw(first), draw(second));
  assert.notDeepEqual(draw(seededRandom(7)), draw(seededRandom(8)));
});

test("every edit kind changes a representative file", () => {
  for (const [kind, edit] of Object.entries(EDIT_KINDS)) {
    const next = edit({
      text: SAMPLE,
      path: "/p/Widget.tsx",
      others: ["/p/helper.ts"],
      rand: seededRandom(3),
      serial: 4
    });
    assert.notEqual(next, null, kind);
    assert.notEqual(next, SAMPLE, kind);
  }
});

test("body keeps every byte offset; prefix shifts them; an import-free file has nothing to remove", () => {
  const ctx = { text: SAMPLE, path: "/p/Widget.tsx", others: [], rand: seededRandom(1), serial: 0 };
  assert.equal(EDIT_KINDS.body(ctx).length, SAMPLE.length);
  assert.ok(EDIT_KINDS.prefix(ctx).length > SAMPLE.length);
  assert.equal(EDIT_KINDS["remove-import"]({ ...ctx, text: "export const a = 1;\n" }), null);
  assert.equal(EDIT_KINDS["add-import"](ctx), null);
});

test("every edited file is restored byte-exact, and a journal restores a killed run", () => {
  const directory = mkdtempSync(join(tmpdir(), "ir-edit-fuzz-test-"));
  try {
    const path = join(directory, "a.ts");
    const original = Buffer.from([0xef, 0xbb, 0xbf, 0x61, 0x0d, 0x0a, 0xff, 0x00, 0x62]);
    writeFileSync(path, original);
    const restorer = new Restorer(join(directory, "journal"));
    restorer.protect(path);
    writeFileSync(path, "edited");
    restorer.protect(path);
    writeFileSync(path, "edited again");
    assert.deepEqual(restorer.restore(), []);
    assert.ok(readFileSync(path).equals(original));
    writeFileSync(path, "edited by a run that was killed");
    assert.equal(restoreJournal(join(directory, "journal")), 1);
    assert.ok(readFileSync(path).equals(original));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the fixture budget names real, flag-free fixture projects", () => {
  for (const id of FIXTURE_PROJECTS) {
    const directory = join(ROOT, "fixtures", id);
    assert.ok(existsSync(join(directory, "tsconfig.json")), id);
    assert.ok(!existsSync(join(directory, ".solid-checker", "runtime.json")), `${id} needs runtime flags`);
  }
});
