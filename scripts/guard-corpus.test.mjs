import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { baselineUsable, changesProblems, digestResults, installFingerprint, movesOf, staleBinary, unnamedMoves } from "./guard/corpus.mjs";

// ADR 1012: the corpus findings of a branch held to main's, on in-memory sweeps.
const ROOT = "/corpus/apps/app-a";
const finding = (kind, id, file, start = 10) => ({ kind, id, primaryLocation: { path: `${ROOT}/${file}`, startByte: start, endByte: start + 5 } });
const sweep = (findings) => [{ app: "app-a", root: ROOT, project: "tsconfig.json", findings }];
const baseResults = sweep([
  finding("violation", "SC2002", "src/a.tsx", 10), finding("violation", "SC2002", "src/a.tsx", 40), finding("violation", "SC1001", "src/b.tsx", 7),
  finding("uncertifiable", "SC1001", "src/b.tsx", 1), finding("uncertifiable", "SC3001", "src/c.tsx", 2), finding("uncertifiable", "SC3001", "src/c.tsx", 3),
]);
const base = digestResults(baseResults);

test("digest: violations exact by project, file, rule id and span; uncertifiable rows counted per project, file and rule id", () => {
  assert.deepEqual(base.violations, ["app-a/tsconfig.json\tsrc/a.tsx\tSC2002\t10-15", "app-a/tsconfig.json\tsrc/a.tsx\tSC2002\t40-45", "app-a/tsconfig.json\tsrc/b.tsx\tSC1001\t7-12"]);
  assert.deepEqual(base.uncertifiable, { "app-a/tsconfig.json\tsrc/b.tsx\tSC1001": 1, "app-a/tsconfig.json\tsrc/c.tsx\tSC3001": 2 });
});

test("a violation that turns uncertifiable, or vanishes, is a lost proof; a place that held no uncertifiable row and holds one is a proved-clean place lost", () => {
  // The laundering: a veto turns a corpus violation into an uncertifiable row; the project's total rises by one, and an ADR's summary says "+1 U".
  const head = digestResults(sweep([
    finding("violation", "SC2002", "src/a.tsx", 10), // the second SC2002 left
    finding("uncertifiable", "SC2002", "src/a.tsx", 40), // ... and a row of its rule stands in its file
    finding("uncertifiable", "SC1001", "src/b.tsx", 1), finding("uncertifiable", "SC3001", "src/c.tsx", 2), finding("uncertifiable", "SC3001", "src/c.tsx", 3),
    finding("uncertifiable", "SC8003", "src/d.tsx", 9), // a new place
    finding("uncertifiable", "SC3001", "src/c.tsx", 4), // more where some stood: not a loss
  ]));
  const moves = movesOf(base, head);
  assert.deepEqual(moves.lost.map((item) => `${item.file} ${item.id} ${item.start} ${item.kind}`), ["src/a.tsx SC2002 40-45 became-uncertifiable", "src/b.tsx SC1001 7-12 gone"]);
  assert.deepEqual(moves.cleanLost.map((item) => `${item.file} ${item.id}`), ["src/a.tsx SC2002", "src/d.tsx SC8003"]);
  assert.equal(moves.rose, 1);
  assert.equal(moves.added, 0);
});

test("moves into proof and quiet changes are no failure: a violation added, rows gone", () => {
  const head = digestResults(sweep([...baseResults[0].findings.filter((item) => item.id !== "SC3001"), finding("violation", "SC9999", "src/z.tsx", 5)]));
  const moves = movesOf(base, head);
  assert.deepEqual([moves.lost.length, moves.cleanLost.length, moves.added, moves.fell], [0, 0, 1, 2]);
});

test("only a NEW row of corpus-changes.json names a move, by project and rule, spent once, and rows add up", () => {
  const std = { lost: [{ project: "app-a/tsconfig.json", file: "src/a.tsx", id: "SC2002", kind: "gone" }, { project: "app-a/tsconfig.json", file: "src/a.tsx", id: "SC2002", kind: "gone" }], cleanLost: [{ project: "app-a/tsconfig.json", file: "src/d.tsx", id: "SC8003", count: 1 }] };
  const dev = { lost: [], cleanLost: [] };
  const row = (over = {}) => ({ project: "app-a/tsconfig.json", rule: "SC2002", kind: "lost-violation", build: "both", count: 2, adr: "ADR 1012", why: "x", ...over });
  const [first] = unnamedMoves({ std, dev }, []).unnamed;
  assert.deepEqual(unnamedMoves({ std, dev }, []).unnamed.map((item) => `${item.kind} ${item.id} ${item.missing}`), ["lost-violation SC2002 2", "newly-uncertifiable SC8003 1"]);
  assert.equal(first.example, "src/a.tsx");
  assert.deepEqual(unnamedMoves({ std, dev }, [row({ count: 1 })]).unnamed.map((item) => `${item.id} ${item.missing}`), ["SC2002 1", "SC8003 1"]);
  assert.deepEqual(unnamedMoves({ std, dev }, [row(), row({ kind: "newly-uncertifiable", rule: "SC8003", count: 1 })]).unnamed, []);
  assert.equal(unnamedMoves({ std, dev }, [row({ project: "app-b/tsconfig.json" })]).unnamed.length, 2);
  assert.equal(unnamedMoves({ std, dev }, [row({ kind: "newly-uncertifiable" })]).unnamed.length, 2);
  // a row for the other build names nothing; one count is spent across builds
  assert.equal(unnamedMoves({ std, dev }, [row({ build: "dev" })]).unnamed.length, 2);
  const both = { lost: std.lost, cleanLost: [] };
  assert.equal(unnamedMoves({ std: both, dev: both }, [row({ count: 2 })]).unnamed.length, 1);
  assert.equal(unnamedMoves({ std: both, dev: both }, [row({ count: 4 })]).unnamed.length, 0);
});

test("corpus-changes.json is append-only against the base, and a new row needs its shape, an ADR and a why", () => {
  const decisions = '<a id="adr-1012"></a>\n';
  const row = (over = {}) => ({ project: "p/t", rule: "SC2002", kind: "lost-violation", build: "std", count: 1, adr: "ADR 1012", why: "x", ...over });
  assert.deepEqual(changesProblems({ head: { rows: [row()] }, base: { rows: [] }, decisions }), []);
  assert.match(changesProblems({ head: { rows: [] }, base: { rows: [row()] }, decisions })[0], /append-only/);
  assert.match(changesProblems({ head: { rows: [row({ count: 5 })] }, base: { rows: [row()] }, decisions })[0], /append-only/);
  for (const [over, pattern] of [[{ adr: "ADR 9999" }, /not an ADR/], [{ why: "" }, /says no why/], [{ count: 0 }, /count must be/], [{ build: "x" }, /build is std/], [{ kind: "x" }, /kind is lost-violation/], [{ project: "" }, /needs a project/]]) {
    assert.match(changesProblems({ head: { rows: [row(over)] }, base: { rows: [] }, decisions })[0], pattern);
  }
});

test("a baseline built on another machine, or from an older checker, is refused rather than reported as moves", () => {
  const trees = { rust: "aaa", typefacts: "bbb", manifest: "mmm", sweep: "sss" };
  const baseline = { fingerprint: "darwin-arm64-111111111111", checker: trees };
  assert.equal(baselineUsable({ baseline, fingerprint: "darwin-arm64-111111111111", trees }).ok, true);
  assert.match(baselineUsable({ baseline: null, fingerprint: "darwin-arm64-111111111111", trees }).problem, /no corpus baseline for this machine/);
  assert.match(baselineUsable({ baseline, fingerprint: "linux-arm64-111111111111", trees }).problem, /built on darwin-arm64-111111111111, this machine is linux-arm64-111111111111: refused/);
  assert.match(baselineUsable({ baseline, fingerprint: "darwin-arm64-111111111111", trees: { ...trees, rust: "ccc" } }).problem, /\(rust\) other than the merge base's/);
  // another corpus manifest or another sweep script is as stale as another checker
  assert.match(baselineUsable({ baseline, fingerprint: "darwin-arm64-111111111111", trees: { ...trees, manifest: "nnn" } }).problem, /\(manifest\) other than the merge base's/);
  assert.match(baselineUsable({ baseline, fingerprint: "darwin-arm64-111111111111", trees: { ...trees, sweep: "ttt", typefacts: "uuu" } }).problem, /\(typefacts, sweep\)/);
  // a baseline from before the manifest and the sweep entered the key has no such member: stale
  assert.equal(baselineUsable({ baseline: { ...baseline, checker: { rust: "aaa", typefacts: "bbb" } }, fingerprint: "darwin-arm64-111111111111", trees }).ok, false);
});

test("the install fingerprint moves with the platform and with any installed package version, and not with the order of packages", () => {
  const root = mkdtempSync(join(tmpdir(), "corpus-install-"));
  const app = (name, versions) => {
    const modules = join(root, name, "node_modules");
    mkdirSync(modules, { recursive: true });
    writeFileSync(join(modules, ".package-lock.json"), JSON.stringify({ packages: Object.fromEntries(versions.map(([pkg, version]) => [`node_modules/${pkg}`, { version }])) }));
  };
  const manifest = { apps: { one: { installDir: "." } } };
  app("one", [["solid-js", "2.0.0-beta.9"], ["vite", "7.1.0"]]);
  const first = installFingerprint(manifest, root, { platform: "darwin", arch: "arm64" });
  assert.match(first, /^darwin-arm64-[0-9a-f]{12}$/);
  app("one", [["vite", "7.1.0"], ["solid-js", "2.0.0-beta.9"]]);
  assert.equal(installFingerprint(manifest, root, { platform: "darwin", arch: "arm64" }), first);
  assert.notEqual(installFingerprint(manifest, root, { platform: "linux", arch: "arm64" }), first);
  app("one", [["solid-js", "2.0.0-beta.9"], ["vite", "7.1.1"]]);
  assert.notEqual(installFingerprint(manifest, root, { platform: "darwin", arch: "arm64" }), first);
});

test("a release binary older than the sources it was built from is refused (its build id is the constant dev)", () => {
  // The laundering: `node scripts/guard/corpus.mjs record` with the binary of an earlier build would record that checker's findings as HEAD's.
  const sources = [{ file: "rust/a.rs", ms: 1000 }, { file: "rust/crates/b.rs", ms: 3000 }];
  assert.equal(staleBinary({ name: "the release checker", binaryMs: 3000, sources }), null);
  assert.equal(staleBinary({ name: "the release checker", binaryMs: 4000, sources }), null);
  assert.match(staleBinary({ name: "the release checker", binaryMs: 2999, sources }), /the release checker is older than rust\/crates\/b.rs .*build the release checker again/);
  assert.equal(staleBinary({ name: "x", binaryMs: 0, sources: [] }), null);
});

