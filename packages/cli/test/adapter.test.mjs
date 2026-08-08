import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const require = createRequire(import.meta.url);
const plugin = require("../eslint.cjs");

function sourceCode(text) {
  return {
    text,
    getLocFromIndex(index) {
      const lines = text.slice(0, index).split("\n");
      return { line: lines.length, column: lines.at(-1).length };
    }
  };
}

function run(snapshot, filename, text) {
  const reports = [];
  const context = {
    settings: { solidChecker: { snapshot } },
    options: [],
    sourceCode: sourceCode(text),
    filename,
    physicalFilename: filename,
    report(descriptor) {
      reports.push(descriptor);
    }
  };
  plugin.rules.certification.create(context).Program({ type: "Program" });
  return reports;
}

test("exports an Oxlint-compatible certification plugin", () => {
  const exported = require("solid-checker/eslint");
  assert.equal(exported.meta.name, "solid-checker");
  assert.ok(exported.rules.certification);
  assert.equal(
    exported.configs.recommended.rules["solid-checker/certification"],
    "error"
  );
});

test("reports canonical diagnostic content for findings belonging to the linted file", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-"));
  const filename = join(root, "App.tsx");
  const other = join(root, "Other.tsx");
  const findings = [filename, other].map((path, index) => ({
    id: `SC100${index + 1}`,
    rule: "strict-read-untracked",
    kind: "violation",
    severity: "error",
    message: "reactive read outside tracking",
    hint: "Move the read into a tracking scope.",
    primaryLocation: {
      path,
      startByte: 6,
      endByte: 11,
      line: 1,
      column: 7
    },
    evidence: [{ message: "proven component prop" }],
    relatedLocations: [{
      path: other,
      startByte: 0,
      endByte: 5,
      line: 1,
      column: 1
    }]
  }));

  const reports = run({ status: "violation", findings }, filename, "const value = 1;");
  assert.equal(reports.length, 1);
  assert.equal(
    reports[0].data.message,
    "[SC1001] reactive read outside tracking\n\nMove the read into a tracking scope."
  );
  assert.deepEqual(reports[0].loc, {
    start: { line: 1, column: 6 },
    end: { line: 1, column: 11 }
  });
});

test("projects safe same-file fixes and UTF-8 byte ranges", () => {
  const filename = join(mkdtempSync(join(tmpdir(), "solid-checker-adapter-")), "App.tsx");
  const location = {
    path: filename,
    startByte: 4,
    endByte: 9,
    line: 1,
    column: 3
  };
  const reports = run({
    findings: [{
      id: "SC1003",
      rule: "component-props-destructure",
      kind: "violation",
      severity: "error",
      message: "do not destructure props",
      primaryLocation: location,
      fixes: [{
        message: "Keep props",
        applicability: "safe",
        edits: [{ location, newText: "props" }]
      }]
    }]
  }, filename, "😀value");

  const calls = [];
  const edits = reports[0].fix({
    replaceTextRange(range, newText) {
      calls.push({ range, newText });
      return { range, text: newText };
    }
  });
  assert.deepEqual(calls, [{ range: [2, 7], newText: "props" }]);
  assert.equal(edits.length, 1);
  assert.equal(plugin._testing.byteOffsetToIndex("😀value", 4), 2);
});

test("discovers tsconfig and runs native analysis once per project", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-"));
  const sourceRoot = join(root, "src");
  mkdirSync(sourceRoot);
  writeFileSync(join(root, "tsconfig.json"), "{}\n");
  const counter = join(root, "runs.txt");
  const analyzer = join(root, "analyzer.mjs");
  writeFileSync(analyzer, `import { existsSync, readFileSync, writeFileSync } from "node:fs";
const counter = process.argv[2];
const args = process.argv.slice(3);
if (!args.includes("--project") || !args.includes("--format") || !args.includes("json")) {
  process.stderr.write("missing transparent project analysis arguments");
  process.exit(2);
}
const count = existsSync(counter) ? Number(readFileSync(counter, "utf8")) : 0;
writeFileSync(counter, String(count + 1));
process.stdout.write(JSON.stringify({ status: "certified", findings: [] }));
`);

  plugin._testing.snapshotCache.clear();
  const config = {
    command: process.execPath,
    commandArgs: [analyzer, counter]
  };
  for (const name of ["App.tsx", "Other.tsx"]) {
    const filename = join(sourceRoot, name);
    writeFileSync(filename, "export {};\n");
    const context = {
      filename,
      physicalFilename: filename,
      settings: { solidChecker: config },
      options: []
    };
    const snapshot = plugin._testing.loadSnapshot(context);
    assert.equal(snapshot.status, "certified");
    assert.equal(plugin._testing.configuredProject(context, config), join(root, "tsconfig.json"));
  }
  assert.equal(readFileSync(counter, "utf8"), "1");
});

test("caches a failed analysis instead of re-spawning every lint pass", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-"));
  writeFileSync(join(root, "tsconfig.json"), "{}\n");
  const counter = join(root, "runs.txt");
  const analyzer = join(root, "analyzer.mjs");
  writeFileSync(analyzer, `import { existsSync, readFileSync, writeFileSync } from "node:fs";
const counter = process.argv[2];
const count = existsSync(counter) ? Number(readFileSync(counter, "utf8")) : 0;
writeFileSync(counter, String(count + 1));
process.stderr.write("analysis exploded");
process.exit(2);
`);

  plugin._testing.snapshotCache.clear();
  const filename = join(root, "App.tsx");
  writeFileSync(filename, "export {};\n");
  const context = {
    filename,
    physicalFilename: filename,
    settings: { solidChecker: { command: process.execPath, commandArgs: [analyzer, counter] } },
    options: []
  };
  const expected = /analysis failed \(2\): analysis exploded/;
  assert.throws(() => plugin._testing.loadSnapshot(context), expected);
  assert.throws(() => plugin._testing.loadSnapshot(context), expected);
  assert.equal(readFileSync(counter, "utf8"), "1");
  plugin._testing.snapshotCache.clear();
});

test("reuses an ESLint parser project before filesystem discovery", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-"));
  const project = join(root, "tsconfig.eslint.json");
  const context = {
    filename: join(root, "src", "App.tsx"),
    languageOptions: { parserOptions: { project: "tsconfig.eslint.json" } }
  };
  assert.equal(
    plugin._testing.configuredProject(context, { cwd: root }),
    project
  );
});

test("per-rule surface: every catalog identity is an ESLint rule", () => {
  const v1 = JSON.parse(readFileSync(new URL("../lib/rules-v1.json", import.meta.url)));
  const v2 = JSON.parse(readFileSync(new URL("../lib/rules-v2.json", import.meta.url)));
  for (const entry of [...v1.rules, ...v2.rules]) {
    assert.ok(plugin.rules[entry.name], `missing rule ${entry.name}`);
  }
  for (const entry of v1.rules) {
    assert.ok(entry.name.startsWith("v1/"), `v1 catalog entry ${entry.name} must be namespaced`);
  }
  for (const entry of v2.rules) {
    assert.ok(!entry.name.includes("/"), `v2 stays unprefixed: ${entry.name}`);
  }
  // The two dialect configs enable exactly their own catalog, plus the
  // certification switch-off that keeps them composable with `recommended`.
  assert.equal(Object.keys(plugin.configs.v1.rules).length, v1.rules.length + 1);
  assert.equal(Object.keys(plugin.configs.v2.rules).length, v2.rules.length + 1);
  assert.equal(plugin.configs.v1.rules["solid-checker/certification"], "off");
  assert.equal(plugin.configs.v2.rules["solid-checker/certification"], "off");
});

test("recommended followed by a dialect config reports each finding once", () => {
  // Flat config semantics: later configs win per rule, so merging the rule
  // maps in listed order is exactly what ESLint resolves.
  const merged = {
    ...plugin.configs.recommended.rules,
    ...plugin.configs.v1.rules
  };
  assert.equal(merged["solid-checker/certification"], "off");

  const findings = [finding("SC1003", "v1/no-destructure", 0, 2)];
  const reported = [];
  for (const [name, severity] of Object.entries(merged)) {
    if (severity === "off") continue;
    const rule = plugin.rules[name.slice("solid-checker/".length)];
    rule.create(syntheticContext({ findings }, reported)).Program({});
  }
  assert.equal(reported.length, 1);
  assert.match(reported[0].data.message, /SC1003/);
});

test("per-rule surface: a rule reports only the findings it owns", () => {
  const findings = [
    finding("SC1003", "v1/no-destructure", 0, 2),
    finding("SC1001", "v1/strict-read-untracked", 3, 5)
  ];
  const reported = [];
  const context = syntheticContext({ findings }, reported);
  plugin.rules["v1/no-destructure"].create(context).Program({});
  assert.equal(reported.length, 1);
  assert.match(reported[0].data.message, /SC1003/);
});

test("per-rule surface: one snapshot load serves every rule of a dialect", () => {
  plugin._testing.snapshotCache.clear();
  const findings = [finding("SC1003", "v1/no-destructure", 0, 2)];
  const snapshotPath = join(tmpdir(), `solid-checker-adapter-shared-${process.pid}.json`);
  writeFileSync(snapshotPath, JSON.stringify({ findings }));
  const reported = [];
  const base = syntheticContext(undefined, reported);
  base.settings = { solidChecker: { snapshotPath } };
  const before = plugin._testing.snapshotCache.size;
  plugin.rules["v1/no-destructure"].create(base).Program({});
  plugin.rules["v1/strict-read-untracked"].create(base).Program({});
  assert.equal(plugin._testing.snapshotCache.size, before + 1);
  rmSync(snapshotPath);
});

function finding(id, rule, start, end) {
  return {
    id,
    rule,
    kind: "violation",
    severity: "error",
    message: "m",
    primaryLocation: { path: "/tmp/adapter-per-rule.tsx", startByte: start, endByte: end }
  };
}

function syntheticContext(snapshot, reported) {
  return {
    settings: snapshot === undefined ? {} : { solidChecker: { snapshot } },
    options: [],
    physicalFilename: "/tmp/adapter-per-rule.tsx",
    sourceCode: { text: "abcdefgh", getLocFromIndex: index => ({ line: 1, column: index }) },
    report: entry => reported.push(entry)
  };
}
