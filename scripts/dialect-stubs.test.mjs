// A gate nothing has ever seen fail is a gate whose shape nobody knows, and
// `checkDialectStubs` was that: it had grown four arms and the only evidence
// any of them worked was that the real tree passed. Each test here builds one
// broken stub under $TMPDIR and demands the matching complaint -- never the
// real fixtures, which these tests would otherwise have to edit.
//
// The arm that motivated the extraction is `a_major_no_dialect_carries`: unlike
// the other three it is not a silent fallback. A stub naming `solid-js@3.0.0`
// makes the checker *refuse* the project (SC9013, ADR 0110 § 1), so the
// fixture's snapshot records a refusal as though it were the behaviour under
// test.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { dialectStubProblems, solidMajor } from "./lib/dialect-stubs.mjs";

/**
 * A throwaway tree holding one fixture with one stub, and the problems the
 * check reports about it. `tracked` and `majors` are supplied rather than read,
 * so no test depends on this repository's git index or dialect manifests.
 */
const check = ({ version, track = true, majors = new Set([2]) }) => {
  const root = mkdtempSync(join(tmpdir(), "solid-stub-check-"));
  try {
    const stub = join(root, "fixtures/reactive-ir/subject/node_modules/solid-js");
    mkdirSync(stub, { recursive: true });
    const id = "fixtures/reactive-ir/subject/node_modules/solid-js/package.json";
    if (version !== undefined) {
      writeFileSync(
        join(root, id),
        typeof version === "string" && version.startsWith("{")
          ? version
          : JSON.stringify({ name: "solid-js", version })
      );
    }
    return dialectStubProblems({
      projectRoot: root,
      groups: ["reactive-ir"],
      tracked: new Set(track ? [id] : []),
      majors,
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test("a stub naming a carried major is the case with nothing to say", () => {
  assert.deepEqual(check({ version: "2.0.0-rc.3" }), []);
});

test("a major no dialect carries is refused, not defaulted", () => {
  const [problem, ...rest] = check({ version: "3.0.0" });
  assert.deepEqual(rest, []);
  assert.match(problem, /names major 3, which no carried dialect models/);
  assert.match(problem, /SC9013/);
});

// Major 0 is the same answer, and worth its own case because a `0.0.0`
// placeholder is the shape a half-written stub actually takes. It is still a
// version, so it is still a contradicted answer about an install rather than
// an absence.
test("a placeholder 0.0.0 is a contradicted answer, not an absence", () => {
  assert.match(check({ version: "0.0.0" })[0], /names major 0/);
});

test("an added dialect makes its major acceptable without editing the check", () => {
  assert.deepEqual(check({ version: "3.0.0", majors: new Set([2, 3]) }), []);
});

test("a version string that is not a version falls back silently", () => {
  assert.match(check({ version: "workspace:*" })[0], /is not a version/);
});

test("an empty node_modules/solid-js directory reports the missing manifest", () => {
  assert.match(check({ version: undefined })[0], /missing/);
});

test("unparseable JSON is reported as itself, not as a missing version", () => {
  const [problem] = check({ version: "{not json" });
  assert.match(problem, /unparseable/);
});

test("a stub absent from the git index names the .gitignore lines to add", () => {
  const problems = check({ version: "2.0.0-rc.3", track: false });
  assert.equal(problems.length, 1);
  assert.match(problems[0], /not tracked by git/);
  assert.match(problems[0], /!fixtures\/reactive-ir\/subject\/node_modules\//);
});

test("the major parse matches Version::for_solid_js", () => {
  assert.equal(solidMajor("1.9.14"), 1);
  assert.equal(solidMajor("2.0.0-rc.0"), 2);
  assert.equal(solidMajor("^1.8.0"), 1);
  assert.equal(solidMajor("v2.0.0"), 2);
  assert.equal(solidMajor("3.0.0"), 3);
  assert.equal(solidMajor("0.5.0"), 0);
  assert.equal(solidMajor("workspace:*"), null);
  assert.equal(solidMajor(""), null);
  assert.equal(solidMajor(undefined), null);
});

// The installation review also reads `@solidjs/signals` and `@solidjs/web`
// beside a `solid-js` stub, so an untracked or versionless companion moves the
// store, `until`, `omit` and `dynamic` answers only in CI. Absent is fine: the
// notice states an unresolved signals, and web is not asked about.
test("a companion stub beside solid-js is held to presence, parse and tracking", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-stub-check-"));
  try {
    const nodeModules = join(root, "fixtures/reactive-ir/subject/node_modules");
    const solid = "fixtures/reactive-ir/subject/node_modules/solid-js/package.json";
    const signals = "fixtures/reactive-ir/subject/node_modules/@solidjs/signals/package.json";
    const web = "fixtures/reactive-ir/subject/node_modules/@solidjs/web/package.json";
    mkdirSync(join(nodeModules, "solid-js"), { recursive: true });
    writeFileSync(join(root, solid), JSON.stringify({ name: "solid-js", version: "2.0.0-rc.3" }));
    const problems = (tracked) =>
      dialectStubProblems({
        projectRoot: root,
        groups: ["reactive-ir"],
        tracked: new Set(tracked),
        majors: new Set([2]),
      });
    assert.deepEqual(problems([solid]), [], "no companion at all is not a problem");
    mkdirSync(join(nodeModules, "@solidjs/signals"), { recursive: true });
    writeFileSync(join(root, signals), JSON.stringify({ name: "@solidjs/signals" }));
    mkdirSync(join(nodeModules, "@solidjs/web"), { recursive: true });
    writeFileSync(join(root, web), JSON.stringify({ name: "@solidjs/web", version: "2.0.0-rc.3" }));
    const found = problems([solid, signals]);
    assert.equal(found.length, 2, found.join("\n"));
    assert.match(found[0], /@solidjs\/signals\/package\.json: no "version"/);
    assert.match(found[1], /@solidjs\/web\/package\.json: not tracked/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
