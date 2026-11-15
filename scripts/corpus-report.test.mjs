// scripts/lib/corpus-report.mjs: a sweep report is written and read a project at a time, so a corpus whose report is longer than V8's longest
// string (`buffer.constants.MAX_STRING_LENGTH`) still sweeps. The writer keeps the bytes `JSON.stringify(report, null, 2)` made; the reader
// gives what `JSON.parse` gave; the sweep script, run over a corpus whose report is over the limit, writes it and `--order-from` reads it.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { constants } from "node:buffer";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, describe, test } from "vitest";

import { cleanReport, fakeChecker } from "./lib/assumed-fixtures.mjs";
import { readReportFile, writeReportFile } from "./lib/corpus-report.mjs";

const root = mkdtempSync(join(tmpdir(), "corpus-report-test-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));

/** A row whose text exercises the scanner: braces, brackets, quotes, backslashes, a comma and a colon inside strings, multi-byte text, nesting. */
const awkward = (index) => ({
  app: `app-${index}`,
  project: "tsconfig.json",
  wallMs: index * 7,
  note: `a "quoted" } brace ] bracket , comma : colon \\ backslash \\" ${"é€😀"} line\nbreak\ttab`,
  nested: { list: [1, 2.5, -3e2, true, false, null, "x"], empty: {}, none: [], deeper: [[{ a: "}" }]] },
  findings: [{ id: "SC1002", kind: "uncertifiable", primaryLocation: { path: "/a/b.tsx", startByte: index, endByte: index + 1 } }],
});

const head = () => ({ checker: "/bin/solid-checker-rust", checkerSha256: "ab".repeat(32), configuration: { runtimeTarget: "browser", daemon: false }, projectCount: 3, findingKinds: { uncertifiable: 3 } });

describe("the same bytes and the same object as JSON.stringify and JSON.parse", () => {
  for (const count of [0, 1, 2, 9]) {
    test(`${count} results`, () => {
      const results = Array.from({ length: count }, (_, index) => awkward(index));
      const path = join(root, `same-${count}.json`);
      writeReportFile(path, head(), results);
      assert.equal(readFileSync(path, "utf8"), `${JSON.stringify({ ...head(), results }, null, 2)}\n`);
      assert.deepEqual(readReportFile(path), JSON.parse(readFileSync(path, "utf8")));
    });
  }

  test("an empty head", () => {
    const path = join(root, "empty-head.json");
    writeReportFile(path, {}, [awkward(1)]);
    assert.equal(readFileSync(path, "utf8"), `${JSON.stringify({ results: [awkward(1)] }, null, 2)}\n`);
    assert.deepEqual(readReportFile(path), { results: [awkward(1)] });
  });

  test("a head that carries `results` is refused", () => {
    assert.throws(() => writeReportFile(join(root, "refused.json"), { results: [] }, []), /carries `results`/);
  });

  test("every chunk size, down to a byte, reads the same report (values cross chunk edges)", () => {
    const results = Array.from({ length: 5 }, (_, index) => awkward(index));
    const path = join(root, "chunks.json");
    writeReportFile(path, head(), results);
    const expected = JSON.parse(readFileSync(path, "utf8"));
    for (const chunkSize of [1, 2, 3, 7, 64, 1000, 1 << 20]) assert.deepEqual(readReportFile(path, { chunkSize }), expected, `chunk ${chunkSize}`);
  });

  test("`onResult` hands over each project and leaves `results` out of the returned object", () => {
    const results = Array.from({ length: 4 }, (_, index) => awkward(index));
    const path = join(root, "stream.json");
    writeReportFile(path, head(), results);
    const seen = [];
    const rest = readReportFile(path, { onResult: (row) => seen.push(row) });
    assert.deepEqual(seen, results);
    assert.deepEqual(rest, head());
  });

  test("a report written by `JSON.stringify` (the reports of before) is read too, whatever its layout", () => {
    const results = [awkward(1), awkward(2)];
    for (const [name, text] of [["pretty", JSON.stringify({ ...head(), results }, null, 2)], ["compact", JSON.stringify({ ...head(), results })], ["members after results", JSON.stringify({ results, after: { n: 1 } })]]) {
      const path = join(root, `old-${name.replace(/ /g, "-")}.json`);
      writeFileSync(path, text);
      assert.deepEqual(readReportFile(path), JSON.parse(text), name);
    }
  });

  test("a truncated file is a syntax error, not a short report", () => {
    const path = join(root, "truncated.json");
    writeReportFile(path, head(), [awkward(1), awkward(2)]);
    const text = readFileSync(path, "utf8");
    writeFileSync(path, text.slice(0, text.length - 30));
    assert.throws(() => readReportFile(path), SyntaxError);
  });
});

// A report over V8's longest string. Each project carries one long plain string; the rows are written by the writer, which never holds the whole.
describe("a report longer than V8's longest string", () => {
  const rowBytes = 46 * 1024 * 1024;
  const rows = Math.ceil(constants.MAX_STRING_LENGTH / rowBytes) + 1;
  const bulk = (index) => ({ app: `app-${index}`, project: "tsconfig.json", wallMs: 1000 - index, filler: "x".repeat(rowBytes), findings: [] });

  test("is written and read back, in chunks", { timeout: 300000 }, () => {
    // About 550 MB in a directory of its own, removed whatever happens.
    const dir = mkdtempSync(join(root, "huge-"));
    try {
      const path = join(dir, "huge.json");
      writeReportFile(path, head(), Array.from({ length: rows }, (_, index) => bulk(index)));
      assert.ok(statSync(path).size > constants.MAX_STRING_LENGTH, "the report is over the longest string");
      // The control: a whole-file read cannot give this report back. How it fails depends on the machine (ERR_STRING_TOO_LONG where the read
      // is attempted, ENOMEM where the buffer cannot be allocated), so the test asserts only that it throws.
      assert.throws(() => readFileSync(path, "utf8"), Error, "a whole-file read is the failure this module avoids");
      const seen = [];
      const rest = readReportFile(path, { onResult: (row) => seen.push([row.app, row.wallMs, row.filler.length, row.filler.slice(-3)]) });
      assert.deepEqual(rest, head());
      assert.equal(seen.length, rows);
      assert.deepEqual(seen[0], ["app-0", 1000, rowBytes, "xxx"]);
      assert.deepEqual(seen.at(-1), [`app-${rows - 1}`, 1000 - (rows - 1), rowBytes, "xxx"]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("the sweep script writes it, and `--order-from` reads it back", { timeout: 600000 }, () => {
    // `rows` projects, each with a checker answer of about 46 MB: the report the sweep ends up with is over the limit, as the corpus's now is.
    // The corpus, the checker's answer (46 MB) and the two reports (about 550 MB each, the second replacing the first) live in a directory of their own, removed whatever happens.
    const dir = mkdtempSync(join(root, "sweep-"));
    try {
      const apps = join(dir, "apps");
      const projects = Array.from({ length: rows }, (_, index) => ({ app: `app${index}`, root: `app${index}`, project: "tsconfig.json" }));
      for (const project of projects) {
        mkdirSync(join(apps, project.root), { recursive: true });
        writeFileSync(join(apps, project.root, "tsconfig.json"), "{}\n");
      }
      const commit = "35c4cc6b7c932f5dee5815b37d3601e7dff90b58";
      writeFileSync(join(dir, "manifest.json"), JSON.stringify({
        configuration: { runtimeTarget: "browser", runtimeBuild: "development", rendering: "csr" },
        apps: Object.fromEntries(projects.map((project) => [project.app, { repository: `https://example.test/${project.app}`, commit }])),
        projects,
      }));
      writeFileSync(join(dir, "typefacts"), "producer\n");
      const answer = cleanReport();
      // One long string in the one finding: the sweep keeps `findings` whole, so the report holds `rows` copies.
      answer.findings[0].message = "y".repeat(rowBytes);
      const checker = fakeChecker(dir, answer);
      const out = join(dir, "sweep.json");
      const run = (extra) => spawnSync(process.execPath, [
        new URL("./corpus-sweep.mjs", import.meta.url).pathname, "--manifest", join(dir, "manifest.json"), "--apps-root", apps, "--checker", checker,
        "--typefacts", join(dir, "typefacts"), "--jobs", "2", "--out", out, ...extra,
      ], { encoding: "utf8", maxBuffer: 1 << 26, env: { ...process.env, NODE_OPTIONS: "--max-old-space-size=8000" } });
      const first = run([]);
      assert.equal(first.status, 0, first.stderr.slice(-2000));
      assert.ok(statSync(out).size > constants.MAX_STRING_LENGTH, "the sweep report is over the longest string");
      const head1 = JSON.parse(first.stdout);
      assert.equal(head1.projectCount, rows);
      assert.equal(head1.findingCount, rows);
      let count = 0;
      const rest = readReportFile(out, { onResult: (row) => { count += 1; assert.equal(row.findings[0].message.length, rowBytes); } });
      assert.equal(count, rows);
      assert.equal(rest.projectCount, rows);
      // The second sweep takes the first as its order: it reads the over-limit report and runs.
      const second = run(["--order-from", out]);
      assert.equal(second.status, 0, second.stderr.slice(-2000));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
