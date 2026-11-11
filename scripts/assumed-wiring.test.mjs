// ADR 0950: the gates below read the checker's JSON report, and nothing can import them or run them in a unit test: each is a top-level script
// that needs what only its own gate provisions (the audited oracle installs, a built WASM package, a corpus). Their intake is held by reading
// their source, as `u-blockers.test.mjs` holds coverage.mjs's blocker string: the report goes through `refuseAssumed` before anything is read
// from it, and no raw `JSON.parse` of the checker's output is left beside it. What `refuseAssumed` does is `assumed.test.mjs`'s; what a
// consumer does when it throws is each consumer's own test (the importable ones feed their real intake, the runnable ones run).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

/**
 * `imports`: where the script reads `refuseAssumed` from. `guarded`: source that must appear (each snippet, as many times as given). `order`: pieces
 * that must stand in this order (the parse, the refusal, the first use). `bare`: raw parses of the checker's output that must not be anywhere.
 */
const GATES = [
  {
    path: "scripts/ownership-gate.mjs",
    imports: "./lib/assumed.mjs",
    guarded: [['refuseAssumed(JSON.parse(output), "scripts/ownership-gate.mjs")', 1]],
    bare: [/(?<!refuseAssumed\()JSON\.parse\(output\)/],
  },
  {
    path: "scripts/obligation-audit.mjs",
    imports: "./lib/assumed.mjs",
    guarded: [['refuseAssumed(JSON.parse(output), "scripts/obligation-audit.mjs")', 1]],
    bare: [/(?<!refuseAssumed\()JSON\.parse\(output\)/],
  },
  {
    // `runNative` and `runNativeRaw`: the native reference the owned (WASM) check is held to.
    path: "scripts/wasm-parity.mjs",
    imports: "./lib/assumed.mjs",
    guarded: [['refuseAssumed(JSON.parse(result.stdout.toString("utf8")), "scripts/wasm-parity.mjs")', 2]],
    bare: [/(?<!refuseAssumed\()JSON\.parse\(result\.stdout\.toString\("utf8"\)\)/],
  },
  {
    // A refusal must not read as "the result is gone" (`return false`): it is rethrown past the catch that says so.
    path: "scripts/repro-reduce.mjs",
    imports: "./lib/assumed.mjs",
    guarded: [
      ['payload = refuseAssumed(JSON.parse(run.stdout), "scripts/repro-reduce.mjs")', 1],
      ["if (error instanceof AssumedReportError) throw error;", 1],
    ],
    bare: [/(?<!refuseAssumed\()JSON\.parse\(run\.stdout\)/],
  },
  {
    path: "scripts/mutants.mjs",
    imports: "./lib/assumed.mjs",
    guarded: [['refuseAssumed(payload, "scripts/mutants.mjs")', 1]],
    order: [["payload = JSON.parse(run.stdout)", 'refuseAssumed(payload, "scripts/mutants.mjs")', "assert(Array.isArray(payload.findings)"]],
  },
  {
    path: "scripts/flip/check.mjs",
    imports: "../lib/assumed.mjs",
    guarded: [['refuseAssumed(out, "scripts/flip/check.mjs")', 1]],
    order: [["out = JSON.parse(r.stdout)", 'refuseAssumed(out, "scripts/flip/check.mjs")', "judgeWitness(w, out)"]],
  },
];

const occurrences = (text, snippet) => text.split(snippet).length - 1;

for (const { path, imports, guarded = [], order = [], bare = [] } of GATES) {
  test(`${path} refuses a what-if report before it reads anything from it`, () => {
    const text = read(path);
    assert.match(text, new RegExp(`import \\{[^}]*\\brefuseAssumed\\b[^}]*\\} from "${imports.replaceAll(/[./]/g, "\\$&")}";`), "imports refuseAssumed from the shared helper");
    for (const [snippet, times] of guarded) assert.equal(occurrences(text, snippet), times, snippet);
    for (const pieces of order) {
      const at = pieces.map((piece) => text.indexOf(piece));
      assert.ok(at.every((index) => index >= 0), `${pieces.join(" ... ")}: every piece is there`);
      assert.deepEqual(at, [...at].sort((a, b) => a - b), `${pieces.join(" ... ")}: in this order`);
    }
    for (const pattern of bare) assert.doesNotMatch(text, pattern, "a raw parse of the checker's output");
  });
}
