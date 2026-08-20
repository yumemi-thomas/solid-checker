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
import { Linter } from "eslint";

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

test("runs as a flat-config plugin on ESLint 10", () => {
  const linter = new Linter();
  const messages = linter.verify(
    "const answer = 42;",
    [{
      plugins: { "solid-checker": plugin },
      settings: { solidChecker: { snapshot: { status: "certified", findings: [] } } },
      rules: { "solid-checker/certification": "error" }
    }],
    { filename: "App.js" }
  );
  assert.deepEqual(messages, []);
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
    "[SC1001] reactive read outside tracking\n\nMove the read into a tracking scope." +
      "\n\nDocs: https://github.com/yumemi-thomas/solid-checker/blob/main/docs/rules/strict-read-untracked.md"
  );
  assert.deepEqual(reports[0].loc, {
    start: { line: 1, column: 6 },
    end: { line: 1, column: 11 }
  });
});

test("certification diagnostics link directly to their rule documentation", () => {
  assert.equal(
    plugin._testing.findingMessage({
      id: "SC1003",
      rule: "v1/no-destructure",
      message: "do not destructure reactive objects"
    }),
    "[SC1003] do not destructure reactive objects\n\n" +
      "Docs: https://github.com/yumemi-thomas/solid-checker/blob/main/docs/rules/v1/no-destructure.md"
  );
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
      rule: "no-destructure",
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

test("per-rule surface: every discovered catalog identity is an ESLint rule", () => {
  const catalogs = Object.values(plugin._testing.manifests);
  const v1 = catalogs.find(catalog => catalog.dialect === "solid-v1");
  const v2 = catalogs.find(catalog => catalog.dialect === "solid-v2");
  assert.deepEqual(
    catalogs.map(catalog => catalog.dialect).sort(),
    ["solid-v1", "solid-v2"]
  );
  for (const entry of [...v1.rules, ...v2.rules]) {
    assert.ok(plugin.rules[entry.name], `missing rule ${entry.name}`);
  }
  for (const entry of v1.rules) {
    assert.ok(entry.name.startsWith("v1/"), `v1 catalog entry ${entry.name} must be namespaced`);
  }
  for (const entry of v2.rules) {
    assert.ok(!entry.name.includes("/"), `v2 stays unprefixed: ${entry.name}`);
  }
  assert.equal(v1.namespace, "v1");
  assert.equal(v2.namespace, "");
  // The two dialect configs enable exactly their default-enabled catalog, plus the
  // certification switch-off that keeps them composable with `recommended`.
  assert.equal(
    Object.keys(plugin.configs.v1.rules).length,
    v1.rules.filter(entry => entry.defaultEnabled).length + 1
  );
  assert.equal(
    Object.keys(plugin.configs.v2.rules).length,
    v2.rules.filter(entry => entry.defaultEnabled).length + 1
  );
  assert.equal(plugin.configs.v1.rules["solid-checker/certification"], "off");
  assert.equal(plugin.configs.v2.rules["solid-checker/certification"], "off");
});

test("preference configs and recommendation metadata follow generated catalogs", () => {
  for (const catalog of Object.values(plugin._testing.manifests)) {
    const preferences = catalog.rules.filter(entry => entry.presets.includes("preferences"));
    const config = plugin.configs[`preferences-${catalog.config}`];
    assert.deepEqual(config.settings.solidChecker.preset, ["preferences"]);
    assert.deepEqual(
      Object.keys(config.rules).sort(),
      preferences.map(entry => `solid-checker/${entry.name}`).sort()
    );
    for (const entry of catalog.rules) {
      assert.equal(
        plugin.rules[entry.name].meta.docs.recommended,
        entry.defaultEnabled && !entry.uncertifiable
      );
      assert.equal(
        `solid-checker/${entry.name}` in plugin.configs[catalog.config].rules,
        entry.defaultEnabled
      );
    }
  }
  assert.equal(plugin.configs["preferences-v2"].settings.solidChecker.dialect, undefined);
});

test("adapter presets and enabled rules are normalized into argv and cache identity", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-preferences-"));
  const project = join(root, "tsconfig.json");
  const calls = join(root, "calls.txt");
  const analyzer = join(root, "analyzer.mjs");
  writeFileSync(project, "{}\n");
  writeFileSync(analyzer, `import { appendFileSync } from "node:fs";
appendFileSync(process.argv[2], JSON.stringify(process.argv.slice(3)) + "\\n");
process.stdout.write(JSON.stringify({ status: "certified", findings: [] }));
`);
  const context = (preset, enableRule) => ({
    filename: join(root, "App.tsx"),
    physicalFilename: join(root, "App.tsx"),
    settings: { solidChecker: {
      command: process.execPath,
      commandArgs: [analyzer, calls],
      project,
      preset,
      enableRule
    } },
    options: []
  });
  plugin._testing.snapshotCache.clear();
  plugin._testing.loadSnapshot(context(["b", "a", "a"], ["prefer-show", "prefer-show"]));
  plugin._testing.loadSnapshot(context(["a", "b"], ["prefer-show"]));
  plugin._testing.loadSnapshot(context(["preferences"], ["prefer-show"]));
  const invocations = readFileSync(calls, "utf8").trim().split("\n").map(JSON.parse);
  assert.equal(invocations.length, 2);
  assert.deepEqual(invocations[0].slice(-6), [
    "--preset", "a", "--preset", "b", "--enable-rule", "prefer-show"
  ]);
  assert.deepEqual(invocations[1].slice(-4), [
    "--preset", "preferences", "--enable-rule", "prefer-show"
  ]);
  plugin._testing.snapshotCache.clear();
});

test("an explicitly configured default-disabled ESLint rule enables native analysis", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-explicit-"));
  const project = join(root, "tsconfig.json");
  const calls = join(root, "calls.txt");
  const analyzer = join(root, "analyzer.mjs");
  const filename = join(root, "App.tsx");
  writeFileSync(project, "{}\n");
  writeFileSync(filename, "export {};\n");
  writeFileSync(analyzer, `import { writeFileSync } from "node:fs";
writeFileSync(process.argv[2], JSON.stringify(process.argv.slice(3)));
process.stdout.write(JSON.stringify({ status: "certified", findings: [] }));
`);
  const context = {
    filename,
    physicalFilename: filename,
    settings: { solidChecker: {
      command: process.execPath,
      commandArgs: [analyzer, calls],
      project
    } },
    options: [],
    sourceCode: sourceCode("export {};\n"),
    report() {}
  };
  plugin._testing.snapshotCache.clear();
  const listeners = plugin.rules["v1/prefer-classlist"].create(context);
  listeners.Program({ type: "Program" });
  listeners["Program:exit"]();
  const args = JSON.parse(readFileSync(calls, "utf8"));
  assert.deepEqual(args.slice(-2), ["--enable-rule", "v1/prefer-classlist"]);
  plugin._testing.snapshotCache.clear();
});

test("deprecated rule keys delegate without entering dialect presets", () => {
  for (const [oldName, currentName] of plugin._testing.deprecatedRuleKeys) {
    const rule = plugin.rules[oldName];
    assert.ok(rule, `missing deprecated rule ${oldName}`);
    assert.equal(rule.meta.deprecated, true);
    assert.deepEqual(rule.meta.replacedBy, [currentName]);
    assert.ok(plugin.rules[currentName], `missing replacement ${currentName}`);
    for (const config of [plugin.configs.v1, plugin.configs.v2]) {
      assert.ok(!(`solid-checker/${oldName}` in config.rules));
    }
  }
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
  lintPass(enabledRules(merged), syntheticContext({ findings }, reported));
  assert.equal(reported.length, 1);
  assert.match(reported[0].data.message, /SC1003/);
});

test("a dialect config followed by recommended reports each finding once", () => {
  // Reverse listing: `recommended` wins the certification entry, so both the
  // per-rule rules and certification are enabled for the same pass. The
  // per-file registry has to keep certification from re-reporting what the
  // per-rule rules own.
  const merged = {
    ...plugin.configs.v1.rules,
    ...plugin.configs.recommended.rules
  };
  assert.equal(merged["solid-checker/certification"], "error");

  const findings = [
    finding("SC1003", "v1/no-destructure", 0, 2),
    finding("SC1001", "v1/strict-read-untracked", 3, 5)
  ];
  const reported = [];
  lintPass(enabledRules(merged), syntheticContext({ findings }, reported));
  assert.equal(reported.length, 2);
  const ids = reported.map(entry => entry.data.message.slice(1, 7)).sort();
  assert.deepEqual(ids, ["SC1001", "SC1003"]);
});

test("certification alone still reports every finding", () => {
  const findings = [
    finding("SC1003", "v1/no-destructure", 0, 2),
    finding("SC1001", "v1/strict-read-untracked", 3, 5)
  ];
  const reported = [];
  lintPass(["certification"], syntheticContext({ findings }, reported));
  assert.equal(reported.length, 2);
});

test("per-rule registrations do not leak into a later certification-only pass", () => {
  // A persistent ESLint server can lint the same file under a per-rule
  // config, then again after the config dropped to certification only. The
  // second pass must report everything: registrations live for one pass.
  const findings = [
    finding("SC1003", "v1/no-destructure", 0, 2),
    finding("SC1001", "v1/strict-read-untracked", 3, 5)
  ];
  const first = [];
  lintPass(enabledRules(plugin.configs.v1.rules), syntheticContext({ findings }, first));
  assert.equal(first.length, 2);
  assert.equal(plugin._testing.ownedRules.size, 0);

  const second = [];
  lintPass(["certification"], syntheticContext({ findings }, second));
  assert.equal(second.length, 2);
});

test("per-rule surface: a rule reports only the findings it owns", () => {
  const findings = [
    finding("SC1003", "v1/no-destructure", 0, 2),
    finding("SC1001", "v1/strict-read-untracked", 3, 5)
  ];
  const reported = [];
  lintPass(["v1/no-destructure"], syntheticContext({ findings }, reported));
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
  lintPass(["v1/no-destructure", "v1/strict-read-untracked"], base);
  assert.equal(plugin._testing.snapshotCache.size, before + 1);
  rmSync(snapshotPath);
});

// Simulate ESLint's per-file execution model: every enabled rule's create()
// builds its listener map before any traversal event fires, then the Program
// enter event reaches every listener before any Program:exit does.
function lintPass(ruleNames, context) {
  const program = { type: "Program" };
  const listeners = ruleNames.map(name => plugin.rules[name].create(context));
  for (const map of listeners) map.Program?.(program);
  for (const map of listeners) map["Program:exit"]?.(program);
}

function enabledRules(merged) {
  return Object.entries(merged)
    .filter(([, severity]) => severity !== "off")
    .map(([name]) => name.slice("solid-checker/".length));
}

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
