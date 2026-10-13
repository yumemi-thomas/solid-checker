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
import { test } from "vitest";
import { Linter } from "eslint";

const require = createRequire(import.meta.url);
const plugin = require("../eslint.cjs");

test("inferred host cache identity follows config, roots and sources", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-inferred-host-"));
  try {
    mkdirSync(join(root, "src"));
    const project = join(root, "tsconfig.json");
    writeFileSync(project, "{}");
    writeFileSync(join(root, "package.json"), JSON.stringify({ private: true, scripts: { build: "vite build" } }));
    const config = join(root, "vite.config.ts");
    writeFileSync(config, "export default {};");
    const filename = join(root, "src", "main.ts");
    writeFileSync(filename, "export {};");
    const counter = join(root, "count.txt");
    const analyzer = join(root, "analyzer.mjs");
    writeFileSync(analyzer, `import {readFileSync, writeFileSync, existsSync} from 'node:fs';
const path = process.argv[2];
writeFileSync(path, String(existsSync(path) ? Number(readFileSync(path, 'utf8')) + 1 : 1));
process.stdout.write(JSON.stringify({status:'certified',findings:[]}));`);
    const context = runtime => ({ filename, physicalFilename: filename, settings: { solidChecker: { project, command: process.execPath, commandArgs: [analyzer, counter], ...(runtime ? { runtime } : {}) } } });
    plugin._testing.snapshotCache.clear();
    plugin._testing.loadSnapshot(context());
    plugin._testing.loadSnapshot(context());
    assert.equal(readFileSync(counter, "utf8"), "2", "native revalidates installed plugin closure on every inferred request");
    writeFileSync(join(root, "src", "entry-server.tsx"), '"use server";');
    plugin._testing.loadSnapshot(context());
    assert.equal(readFileSync(counter, "utf8"), "3");
    writeFileSync(config, "export default {ssr:true};");
    plugin._testing.loadSnapshot(context());
    assert.equal(readFileSync(counter, "utf8"), "4");
    plugin._testing.loadSnapshot(context({ target: "browser" }));
    writeFileSync(filename, "export const changed = true;");
    plugin._testing.loadSnapshot(context({ target: "browser" }));
    assert.equal(readFileSync(counter, "utf8"), "5", "explicit target bypasses inference identity");
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

test("Oxlint messages retain native inferred-host evidence", () => {
  const filename = "/project/App.tsx";
  const finding = {
    id: "SC4001", rule: "missing-owner", kind: "violation", message: "missing owner",
    primaryLocation: { path: filename, startByte: 0, endByte: 4 },
    evidence: [{ message: "inferred browser: Vite index.html module entry" }]
  };
  const reports = run({ status: "violation", findings: [finding] }, filename, "call");
  assert.equal(reports[0].messageId, "finding");
  assert.match(reports[0].data.message, /inferred browser: Vite index.html module entry/);
  const baseline = run({ status: "violation", findings: [{ ...finding, evidence: [] }] }, filename, "call");
  assert.doesNotMatch(baseline[0].data.message, /inferred browser/);
});

test("native inference decisions project through the run-note rule", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-host-note-"));
  try {
    const project = join(root, "tsconfig.json");
    const filename = join(root, "App.tsx");
    const analyzer = join(root, "analyzer.mjs");
    writeFileSync(project, "{}");
    writeFileSync(filename, "export {};");
    writeFileSync(analyzer, `process.stderr.write(process.argv[2] + '\\n');
process.stdout.write(JSON.stringify({status:'certified',findings:[]}));`);
    for (const decision of [
      "browser host inferred for 2 scopes from roots /project/src/main.ts#init",
      "browser host not inferred: /project/index.html:4: inline or non-module script"
    ]) {
      plugin._testing.snapshotCache.clear();
      const reports = [];
      const context = {
        filename, physicalFilename: filename, options: [],
        sourceCode: sourceCode("export {};"),
        settings: { solidChecker: { project, command: process.execPath,
          commandArgs: [analyzer, `solid-checker: note: ${decision}`] } },
        report: descriptor => reports.push(descriptor)
      };
      plugin.rules["contract-note"].create(context).Program({ type: "Program" });
      assert.equal(reports.length, 1);
      assert.equal(reports[0].messageId, "notice");
      assert.equal(reports[0].data.message, `[solid-checker note] ${decision}`);
      assert.equal(plugin._testing.loadSnapshot(context).status, "certified");
      assert.deepEqual(plugin._testing.loadSnapshot(context).findings, []);
    }
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

function sourceCode(text) {
  return {
    text,
    getLocFromIndex(index) {
      const lines = text.slice(0, index).split("\n");
      return { line: lines.length, column: lines.at(-1).length };
    }
  };
}

function run(snapshot, filename, text, rule = "certification") {
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
  const created = plugin.rules[rule].create(context);
  created.Program({ type: "Program" });
  created["Program:exit"]?.();
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
  assert.ok(exported.rules["contract-note"]);
  assert.equal(exported.rules["contract-note"].meta.fixable, undefined);
  // Every shipped config carries the note rule, at warn, so a note never
  // fails a lint by itself.
  for (const [name, config] of Object.entries(exported.configs)) {
    assert.equal(config.rules["solid-checker/contract-note"], "warn", name);
  }
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
      rule: "no-destructure",
      message: "do not destructure reactive objects"
    }),
    "[SC1003] do not destructure reactive objects\n\n" +
      "Docs: https://github.com/yumemi-thomas/solid-checker/blob/main/docs/rules/no-destructure.md"
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
  const v2 = catalogs.find(catalog => catalog.dialect === "solid-v2");
  // One catalog ships now (ADR 0110). The adapter still *discovers* catalogs
  // from `lib/rules-solid-v*.json` rather than naming one, so a second dialect
  // needs no adapter change -- which is why this asserts the discovered set
  // rather than hard-coding a single manifest.
  assert.deepEqual(
    catalogs.map(catalog => catalog.dialect).sort(),
    ["solid-v2"]
  );
  for (const entry of v2.rules) {
    assert.ok(plugin.rules[entry.name], `missing rule ${entry.name}`);
    assert.ok(!entry.name.includes("/"), `v2 stays unprefixed: ${entry.name}`);
  }
  assert.equal(v2.namespace, "");
  // The dialect config enables exactly its default-enabled catalog, less the
  // opt-in contract-gap rule (ADR 0248), plus the certification switch-off
  // that keeps it composable with `recommended` and the note rule every
  // shipped config carries.
  assert.equal(
    Object.keys(plugin.configs.v2.rules).length,
    v2.rules.filter(entry => entry.defaultEnabled && entry.name !== "package-contract-incomplete")
      .length + 2
  );
  assert.equal(plugin.configs.v2.rules["solid-checker/package-contract-incomplete"], undefined);
  assert.equal(plugin.configs.v2.rules["solid-checker/certification"], "off");
  assert.equal(plugin.configs.v2.rules["solid-checker/contract-note"], "warn");
});

test("browser configs are the dialect config with the browser runtime target", () => {
  for (const catalog of Object.values(plugin._testing.manifests)) {
    const config = plugin.configs[`browser-${catalog.config}`];
    assert.deepEqual(config.rules, plugin.configs[catalog.config].rules);
    assert.deepEqual(config.settings, { solidChecker: { runtime: { target: "browser" } } });
    assert.equal(plugin.configs[catalog.config].settings, undefined);
  }
});

test("preference configs and recommendation metadata follow generated catalogs", () => {
  for (const catalog of Object.values(plugin._testing.manifests)) {
    const preferences = catalog.rules.filter(entry => entry.presets.includes("preferences"));
    const config = plugin.configs[`preferences-${catalog.config}`];
    assert.deepEqual(config.settings.solidChecker.preset, ["preferences"]);
    assert.deepEqual(
      Object.keys(config.rules).sort(),
      [
        "solid-checker/contract-note",
        ...preferences.map(entry => `solid-checker/${entry.name}`)
      ].sort()
    );
    for (const entry of catalog.rules) {
      assert.equal(
        plugin.rules[entry.name].meta.docs.recommended,
        entry.defaultEnabled && !entry.uncertifiable
      );
      assert.equal(
        `solid-checker/${entry.name}` in plugin.configs[catalog.config].rules,
        entry.defaultEnabled && entry.name !== "package-contract-incomplete"
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

test("adapter forwards explicit runtime conditions and includes them in cache identity", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-runtime-"));
  const project = join(root, "tsconfig.json");
  const calls = join(root, "calls.txt");
  const analyzer = join(root, "analyzer.mjs");
  writeFileSync(project, "{}\n");
  writeFileSync(analyzer, `import { appendFileSync } from "node:fs";
appendFileSync(process.argv[2], JSON.stringify(process.argv.slice(3)) + "\\n");
process.stdout.write(JSON.stringify({ status: "certified", findings: [] }));
`);
  const context = runtime => ({
    filename: join(root, "App.tsx"),
    physicalFilename: join(root, "App.tsx"),
    settings: { solidChecker: {
      command: process.execPath,
      commandArgs: [analyzer, calls],
      project,
      runtime
    } },
    options: []
  });
  plugin._testing.snapshotCache.clear();
  plugin._testing.loadSnapshot(context({
    target: "browser",
    rendering: "csr",
    conditions: ["import", "browser", "import"],
    frameworkTransforms: ["use-server"]
  }));
  plugin._testing.loadSnapshot(context({
    target: "browser",
    rendering: "csr",
    conditions: ["browser", "import"],
    frameworkTransforms: ["use-server"]
  }));
  plugin._testing.loadSnapshot(context({ target: "node", rendering: "string-ssr" }));
  const invocations = readFileSync(calls, "utf8").trim().split("\n").map(JSON.parse);
  assert.equal(invocations.length, 2);
  assert.deepEqual(invocations[0].slice(-12), [
    "--format", "json",
    "--runtime-target", "browser",
    "--rendering", "csr",
    "--runtime-condition", "browser",
    "--runtime-condition", "import",
    "--framework-transform", "use-server"
  ]);
  assert.deepEqual(invocations[1].slice(-4), [
    "--runtime-target", "node",
    "--rendering", "string-ssr"
  ]);
  plugin._testing.snapshotCache.clear();
});

test("an explicitly configured default-enabled ESLint rule needs no native override", () => {
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
  const listeners = plugin.rules["prefer-show"].create(context);
  listeners.Program({ type: "Program" });
  listeners["Program:exit"]();
  const args = JSON.parse(readFileSync(calls, "utf8"));
  assert.ok(!args.includes("--enable-rule"));
  plugin._testing.snapshotCache.clear();
});

test("deprecated rule keys delegate without entering dialect presets", () => {
  for (const [oldName, currentName] of plugin._testing.deprecatedRuleKeys) {
    const rule = plugin.rules[oldName];
    assert.ok(rule, `missing deprecated rule ${oldName}`);
    assert.equal(rule.meta.deprecated, true);
    assert.deepEqual(rule.meta.replacedBy, [currentName]);
    assert.ok(plugin.rules[currentName], `missing replacement ${currentName}`);
    for (const config of [plugin.configs.v2]) {
      assert.ok(!(`solid-checker/${oldName}` in config.rules));
    }
  }
});

test("recommended followed by a dialect config reports each finding once", () => {
  // Flat config semantics: later configs win per rule, so merging the rule
  // maps in listed order is exactly what ESLint resolves.
  const merged = {
    ...plugin.configs.recommended.rules,
    ...plugin.configs.v2.rules
  };
  assert.equal(merged["solid-checker/certification"], "off");

  const findings = [finding("SC1003", "no-destructure", 0, 2)];
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
    ...plugin.configs.v2.rules,
    ...plugin.configs.recommended.rules
  };
  assert.equal(merged["solid-checker/certification"], "error");

  const findings = [
    finding("SC1003", "no-destructure", 0, 2),
    finding("SC1001", "strict-read-untracked", 3, 5)
  ];
  const reported = [];
  lintPass(enabledRules(merged), syntheticContext({ findings }, reported));
  assert.equal(reported.length, 2);
  const ids = reported.map(entry => entry.data.message.slice(1, 7)).sort();
  assert.deepEqual(ids, ["SC1001", "SC1003"]);
});

test("certification alone still reports every finding", () => {
  const findings = [
    finding("SC1003", "no-destructure", 0, 2),
    finding("SC1001", "strict-read-untracked", 3, 5)
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
    finding("SC1003", "no-destructure", 0, 2),
    finding("SC1001", "strict-read-untracked", 3, 5)
  ];
  const first = [];
  lintPass(enabledRules(plugin.configs.v2.rules), syntheticContext({ findings }, first));
  assert.equal(first.length, 2);
  assert.equal(plugin._testing.ownedRules.size, 0);

  const second = [];
  lintPass(["certification"], syntheticContext({ findings }, second));
  assert.equal(second.length, 2);
});

test("per-rule surface: a rule reports only the findings it owns", () => {
  const findings = [
    finding("SC1003", "no-destructure", 0, 2),
    finding("SC1001", "strict-read-untracked", 3, 5)
  ];
  const reported = [];
  lintPass(["no-destructure"], syntheticContext({ findings }, reported));
  assert.equal(reported.length, 1);
  assert.match(reported[0].data.message, /SC1003/);
});

test("per-rule surface: one snapshot load serves every rule of a dialect", () => {
  plugin._testing.snapshotCache.clear();
  const findings = [finding("SC1003", "no-destructure", 0, 2)];
  const snapshotPath = join(tmpdir(), `solid-checker-adapter-shared-${process.pid}.json`);
  writeFileSync(snapshotPath, JSON.stringify({ findings }));
  const reported = [];
  const base = syntheticContext(undefined, reported);
  base.settings = { solidChecker: { snapshotPath } };
  const before = plugin._testing.snapshotCache.size;
  lintPass(["no-destructure", "strict-read-untracked"], base);
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

test("a project-scoped finding is reported on every linted file, not matched by path", () => {
  // The unsupported-runtime refusal is located at the `node_modules/solid-js`
  // manifest that decided the dialect. ESLint never lints that file, so the
  // ordinary path match would drop it and hand the user a clean run over a
  // project that was never analyzed -- the false certification the refusal
  // exists to prevent. Reported on every file instead.
  const refusal = {
    id: "SC9013",
    rule: "unsupported-solid-runtime",
    kind: "uncertifiable",
    severity: "error",
    message: "solid-js 1.9.14 is installed, and this build of solid-checker carries no dialect for it; the project was not analyzed",
    subjectKind: "project",
    primaryLocation: {
      path: "/tmp/app/node_modules/solid-js/package.json",
      startByte: 0,
      endByte: 0
    }
  };
  const snapshot = { status: "uncertifiable", findings: [refusal] };

  const reports = run(snapshot, "/tmp/app/src/App.tsx", "const a = 1;");
  assert.equal(reports.length, 1, "the refusal reaches a file it does not name");
  assert.match(reports[0].data.message, /SC9013/);
  assert.deepEqual(reports[0].loc.start, { line: 1, column: 0 });
  assert.deepEqual(
    reports[0].loc.end,
    { line: 1, column: 0 },
    "the span is this file's origin, never an offset into the manifest's bytes"
  );

  // Every other file too: the whole project is unanalyzed, so no file in it
  // may report clean.
  assert.equal(run(snapshot, "/tmp/app/src/Other.tsx", "const b = 2;").length, 1);

  // And the path match still governs everything that is not project-scoped:
  // a file-scoped finding naming another file stays where it belongs.
  const elsewhere = { ...refusal, subjectKind: "component-props", id: "SC1003" };
  assert.equal(
    run({ status: "violation", findings: [elsewhere] }, "/tmp/app/src/App.tsx", "const a = 1;")
      .length,
    0,
    "dropping the project scope restores ordinary per-file matching"
  );
});

test("a finding collapsed over a package export is reported in every file holding a site", () => {
  // Open-claims SC9005 is one finding per (package, export, open domains) for
  // the whole project: its related locations are further sites, not context.
  // A per-file reporter must still see it in each file that has one, once, at
  // that file's first site.
  const collapsed = {
    id: "SC9005",
    rule: "package-contract-incomplete",
    kind: "uncertifiable",
    severity: "warning",
    message: "the reactivity contract for pkg leaves callbacks unknown for imported export run; " +
      "code whose proof depends on those claims cannot be certified (3 call sites)",
    analysisContext: "unknown-contract-claims:callbacks",
    subjectKind: "package-export",
    primaryLocation: { path: "/tmp/app/App.ts", startByte: 2, endByte: 4 },
    relatedLocations: [
      { path: "/tmp/app/Other.ts", startByte: 8, endByte: 10 },
      { path: "/tmp/app/Other.ts", startByte: 4, endByte: 6 }
    ]
  };
  const snapshot = { status: "uncertifiable", findings: [collapsed] };
  const text = "0123456789ab";
  const rule = "package-contract-incomplete";

  // ADR 0248: the catch-all rule leaves contract gaps out; the opt-in
  // per-rule rule reports them.
  assert.equal(run(snapshot, "/tmp/app/App.ts", text).length, 0);

  const primary = run(snapshot, "/tmp/app/App.ts", text, rule);
  assert.equal(primary.length, 1);
  assert.deepEqual(primary[0].loc.start, { line: 1, column: 2 });

  const other = run(snapshot, "/tmp/app/Other.ts", text, rule);
  assert.equal(other.length, 1, "one report per file, not one per site");
  assert.deepEqual(other[0].loc.start, { line: 1, column: 4 }, "at the file's first site");
  assert.equal(other[0].data.message, primary[0].data.message);

  assert.equal(run(snapshot, "/tmp/app/Unrelated.ts", text, rule).length, 0);

  // The same shape without a site subject keeps ordinary per-file matching:
  // a strict read's related location is its declaration, not a second read.
  const context = { ...collapsed, subjectKind: "" };
  assert.equal(
    run({ status: "uncertifiable", findings: [context] }, "/tmp/app/Other.ts", text, rule).length,
    0
  );
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

function recordingAnalyzer(root, body = "") {
  const calls = join(root, "calls.txt");
  const analyzer = join(root, "analyzer.mjs");
  writeFileSync(analyzer, `import { appendFileSync } from "node:fs";
appendFileSync(process.argv[2], JSON.stringify(process.argv.slice(3)) + "\\n");
${body}
process.stdout.write(JSON.stringify({ status: "certified", findings: [] }));
`);
  const invocations = () => {
    try {
      return readFileSync(calls, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);
    } catch {
      return [];
    }
  };
  return { command: process.execPath, commandArgs: [analyzer, calls], invocations };
}

function flagValue(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? undefined : args[index + 1];
}

test("receiptTrustConfiguration reaches the checker absolute, and its bytes are cache identity", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-trust-"));
  const project = join(root, "app", "tsconfig.json");
  mkdirSync(join(root, "app"));
  writeFileSync(project, "{}\n");
  const trustPath = join(root, "trust.json");
  writeFileSync(trustPath, "{\"issuers\":[]}\n");
  const { command, commandArgs, invocations } = recordingAnalyzer(root);
  // Relative to `cwd`, like `project` and `snapshotPath` -- not to the
  // tsconfig directory the checker runs in.
  const context = settings => ({
    filename: join(root, "app", "App.tsx"),
    physicalFilename: join(root, "app", "App.tsx"),
    settings: { solidChecker: { command, commandArgs, project, cwd: root, ...settings } },
    options: []
  });
  plugin._testing.snapshotCache.clear();
  const trusted = context({
    receiptTrustConfiguration: "trust.json",
    acceptedContracts: "catalog.json"
  });
  plugin._testing.loadSnapshot(trusted);
  plugin._testing.loadSnapshot(trusted);
  assert.equal(invocations().length, 1, "unchanged trust bytes reuse the snapshot");
  assert.equal(flagValue(invocations()[0], "--receipt-trust-configuration"), trustPath);
  assert.equal(flagValue(invocations()[0], "--accepted-contracts"), join(root, "catalog.json"));

  // Same path, new bytes: a persistent ESLint session must not serve the
  // verdict the old trust produced.
  writeFileSync(trustPath, "{\"issuers\":[\"replaced\"]}\n");
  plugin._testing.loadSnapshot(trusted);
  assert.equal(invocations().length, 2, "edited trust bytes re-run the analysis");

  // And no trust is a different identity from any trust.
  plugin._testing.loadSnapshot(context({ acceptedContracts: "catalog.json" }));
  assert.equal(invocations().length, 3);
  assert.equal(flagValue(invocations()[2], "--receipt-trust-configuration"), undefined);
  plugin._testing.snapshotCache.clear();
});

test("an unreadable receiptTrustConfiguration is a clear ESLint error, not an analysis", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-trust-missing-"));
  const project = join(root, "tsconfig.json");
  writeFileSync(project, "{}\n");
  const filename = join(root, "App.js");
  const { command, commandArgs, invocations } = recordingAnalyzer(root);
  plugin._testing.snapshotCache.clear();
  const config = [{
    plugins: { "solid-checker": plugin },
    settings: { solidChecker: {
      command, commandArgs, project, cwd: root, receiptTrustConfiguration: "missing-trust.json"
    } },
    rules: { "solid-checker/certification": "error" }
  }];
  assert.throws(
    () => new Linter({ cwd: root }).verify("export {};\n", config, { filename }),
    error =>
      error.message.includes("settings.solidChecker.receiptTrustConfiguration") &&
      error.message.includes(join(root, "missing-trust.json")) &&
      error.message.includes("ENOENT")
  );
  assert.equal(invocations().length, 0, "the checker never starts on a bad trust path");

  // Not cached: supplying the file recovers in the same process.
  writeFileSync(join(root, "missing-trust.json"), "{}\n");
  assert.deepEqual(new Linter({ cwd: root }).verify("export {};\n", config, { filename }), []);
  assert.equal(invocations().length, 1);
  plugin._testing.snapshotCache.clear();
});

test("a withheld-catalog note reaches ESLint once per linted file, as a warning", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-note-"));
  const project = join(root, "tsconfig.json");
  writeFileSync(project, "{}\n");
  // The native checker's notice, in shape: exit 0, findings on stdout, the
  // note on stderr, which the adapter used to drop.
  const note =
    "solid-checker: note: project catalog /p/.solid-checker/accepted-contracts.json was not read: " +
    "policy-2 receipts need a trusted issuer and no trust configuration was supplied, so the " +
    "contracts for @kobalte/core are not admitted and the analysis proceeds as if the catalog " +
    "was absent; pass --receipt-trust-configuration <trust.json> to admit them";
  const { command, commandArgs, invocations } = recordingAnalyzer(
    root,
    `process.stderr.write(${JSON.stringify(`timing noise\n${note}\n`)});`
  );
  const settings = { solidChecker: { command, commandArgs, project } };
  const lint = (rules, name) => new Linter({ cwd: root }).verify(
    "export const a = 1;\n",
    [{ plugins: { "solid-checker": plugin }, settings, rules }],
    { filename: join(root, name) }
  );
  plugin._testing.snapshotCache.clear();

  for (const name of ["App.js", "Other.js"]) {
    const messages = lint(plugin.configs.recommended.rules, name);
    assert.equal(messages.length, 1, `${name} carries the note`);
    assert.equal(messages[0].ruleId, "solid-checker/contract-note");
    // A note never fails CI by itself, even under `recommended`, whose
    // certification rule is an error.
    assert.equal(messages[0].severity, 1);
    assert.equal(messages[0].line, 1);
    assert.equal(messages[0].column, 1);
    assert.match(messages[0].message, /^\[solid-checker note\] project catalog /);
    assert.ok(!messages[0].message.includes("timing noise"), "only note lines surface");
    assert.match(messages[0].message, /settings\.solidChecker\.receiptTrustConfiguration/);
  }
  assert.equal(invocations().length, 1, "the note rides the cached snapshot");

  // Every shipped config and both listing orders: exactly one note, from the
  // note rule, at warn -- certification, re-enabled or not, never repeats it.
  for (const rules of [
    plugin.configs.v2.rules,
    plugin.configs["preferences-v2"].rules,
    { ...plugin.configs.recommended.rules, ...plugin.configs.v2.rules },
    { ...plugin.configs.v2.rules, ...plugin.configs.recommended.rules }
  ]) {
    const messages = lint(rules, "App.js");
    assert.equal(messages.length, 1);
    assert.equal(messages[0].ruleId, "solid-checker/contract-note");
    assert.equal(messages[0].severity, 1);
  }
  // Certification alone, and the per-rule rules alone, report findings only.
  assert.deepEqual(lint({ "solid-checker/certification": "error" }, "App.js"), []);
  assert.deepEqual(
    lint({ ...plugin.configs.v2.rules, "solid-checker/contract-note": "off" }, "App.js"),
    []
  );
  // Disabling the note rule is the user's choice to hide notes.
  assert.deepEqual(
    lint({ ...plugin.configs.recommended.rules, "solid-checker/contract-note": "off" }, "App.js"),
    []
  );
  // A severity the user raises is theirs too.
  const raised = lint(
    { ...plugin.configs.recommended.rules, "solid-checker/contract-note": "error" },
    "App.js"
  );
  assert.equal(raised.length, 1);
  assert.equal(raised[0].severity, 2);
  assert.equal(invocations().length, 1, "the note rule shares the pass's analysis");

  // A pass that ends in a thrown analysis never reaches Program:exit; a later
  // pass over the same file must still carry the note.
  const broken = join(root, "broken.mjs");
  writeFileSync(broken, "process.stderr.write('exploded'); process.exit(2);\n");
  const failing = [{
    plugins: { "solid-checker": plugin },
    settings: { solidChecker: { command: process.execPath, commandArgs: [broken], project } },
    rules: plugin.configs.recommended.rules
  }];
  const filename = join(root, "Third.js");
  assert.throws(
    () => new Linter({ cwd: root }).verify("export {};\n", failing, { filename }),
    /exploded/
  );
  for (const rules of [plugin.configs.v2.rules, plugin.configs.recommended.rules]) {
    const after = lint(rules, "Third.js");
    assert.equal(after.length, 1, "the note survives a pass that aborted");
    assert.equal(after[0].ruleId, "solid-checker/contract-note");
    assert.match(after[0].message, /^\[solid-checker note\]/);
  }
  plugin._testing.snapshotCache.clear();
});

test("a named catalog that needs trust fails naming the ESLint setting", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-named-"));
  const project = join(root, "tsconfig.json");
  writeFileSync(project, "{}\n");
  const analyzer = join(root, "analyzer.mjs");
  writeFileSync(analyzer, `process.stderr.write("--accepted-contracts c.json: policy-2 acceptance receipt requires authenticated issuer provenance; pass --receipt-trust-configuration <trust.json> naming the issuer that certified it");
process.exit(2);
`);
  const context = {
    filename: join(root, "App.tsx"),
    physicalFilename: join(root, "App.tsx"),
    settings: { solidChecker: {
      command: process.execPath, commandArgs: [analyzer], project, acceptedContracts: "c.json"
    } },
    options: []
  };
  plugin._testing.snapshotCache.clear();
  assert.throws(
    () => plugin._testing.loadSnapshot(context),
    /authenticated issuer provenance[\s\S]*settings\.solidChecker\.receiptTrustConfiguration/
  );
  plugin._testing.snapshotCache.clear();
});
