import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import {
  EDIT_KINDS,
  FIXTURE_PROJECTS,
  RETYPE_FIXTURE_PROJECTS,
  ROOT,
  Restorer,
  notServedByDaemon,
  projectFiles,
  restoreJournal,
  retypeReport,
  seededRandom
} from "./ir-edit-fuzz.mjs";
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
    // Needs a calling file on disk; tested below.
    if (kind === "retype-return") continue;
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

test("retype-return wraps a called export's top-level return and names its callers", () => {
  const directory = mkdtempSync(join(tmpdir(), "ir-edit-fuzz-retype-"));
  try {
    const callee = join(directory, "source.ts");
    const caller = join(directory, "App.tsx");
    const other = join(directory, "Other.tsx");
    const text = "export function makeReader() {\n  if (x) { return 1; }\n  return () => count();\n}\nexport function typed(): number {\n  return 1;\n}\n";
    writeFileSync(callee, text);
    writeFileSync(caller, 'import { makeReader } from "./source";\nconst read = makeReader();\n');
    writeFileSync(other, 'import { makeReader } from "./source";\nexport { makeReader };\n');
    const edited = EDIT_KINDS["retype-return"]({ text, path: callee, others: [caller, other], rand: seededRandom(1) });
    assert.equal(edited.text, text.replace("return () => count();", "return [() => count()] as const;"));
    assert.deepEqual(edited.expect, { retyped: "makeReader", callers: [caller] });
    // No caller calls it: nothing to retype.
    assert.equal(EDIT_KINDS["retype-return"]({ text, path: callee, others: [other], rand: seededRandom(1) }), null);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a retyped caller must be in the retained build's delta unless it had none", () => {
  const expect = { retyped: "f", callers: ["/p/App.tsx"] };
  const line = (delta, entityPaths) => `${JSON.stringify({ reactiveIrDelta: { delta, entityPaths, filePaths: [] } })}\n`;
  assert.equal(retypeReport(expect, line(true, ["/p/source.ts", "/p/App.tsx"])), "");
  assert.match(retypeReport(expect, line(true, ["/p/source.ts"])), /no caller/);
  // A whole-table replacement recomputes every file.
  assert.equal(retypeReport(expect, line(false, ["/p/source.ts"])), "");
  assert.match(retypeReport(expect, ""), /no IR build/);
  // The first line is the retained build's; a later fresh run's does not count.
  assert.match(retypeReport(expect, line(true, ["/p/source.ts"]) + line(true, ["/p/App.tsx"])), /no caller/);
});

test("every retype fixture has a site for the family", () => {
  for (const id of RETYPE_FIXTURE_PROJECTS) {
    const files = projectFiles(join(ROOT, "fixtures", id));
    const sites = files.filter((path) =>
      EDIT_KINDS["retype-return"]({
        text: readFileSync(path, "utf8"),
        path,
        others: files.filter((other) => other !== path),
        rand: seededRandom(1)
      })
    );
    assert.ok(sites.length > 0, id);
    assert.ok(!existsSync(join(ROOT, "fixtures", id, ".solid-checker", "runtime.json")), `${id} needs runtime flags`);
  }
});

test("a step the daemon did not answer is named, never passed", () => {
  const retained = '{"cacheHit":false,"generation":2,"mode":"retained-daemon"}\n';
  assert.equal(notServedByDaemon(`note: browser host not inferred\n${retained}`), null);
  assert.match(
    notServedByDaemon("solid-checker: daemon unavailable (Broken pipe); running one-shot\n"),
    /daemon unavailable \(Broken pipe\)/
  );
  // A one-shot answer with no mode line at all is not the daemon's either.
  assert.match(notServedByDaemon('{"hostStage":"host-discovery-config"}\n'), /no retained-daemon answer/);
  assert.match(notServedByDaemon(""), /no retained-daemon answer/);
});
