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
    assert.equal(readFileSync(counter, "utf8"), "6", "every pass asks native again, explicit target or not");
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
      plugin.rules["package-note"].create(context).Program({ type: "Program" });
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
  assert.equal(exported.rules["package-note"], exported.rules["contract-note"]);
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

test("discovers tsconfig and asks native once per lint pass", () => {
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
  // Two files are two passes; native's daemon makes the repeat cheap.
  assert.equal(readFileSync(counter, "utf8"), "2");
});

test("keeps a failed analysis for its lint pass only", () => {
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
  const context = () => ({
    filename,
    physicalFilename: filename,
    sourceCode: sourceCode("export {};\n"),
    settings: { solidChecker: { command: process.execPath, commandArgs: [analyzer, counter] } },
    options: []
  });
  const expected = /analysis failed \(2\): analysis exploded/;
  const pass = context();
  assert.throws(() => plugin._testing.loadSnapshot(pass), expected);
  assert.throws(() => plugin._testing.loadSnapshot(Object.create(pass)), expected);
  assert.equal(readFileSync(counter, "utf8"), "1", "the rules of one pass share the failure");
  assert.throws(() => plugin._testing.loadSnapshot(context()), expected);
  assert.equal(readFileSync(counter, "utf8"), "2", "a later pass tries again");
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
  // The dialect config enables exactly its default-enabled catalog, plus the
  // certification switch-off that keeps it composable with `recommended` and
  // the note rule every shipped config carries. An obligation (SC9005, SC9012)
  // has no manifest entry and so no key (ADR 0440).
  assert.equal(
    Object.keys(plugin.configs.v2.rules).length,
    v2.rules.filter(entry => entry.defaultEnabled).length + 2
  );
  for (const obligation of plugin._testing.retiredRuleKeys) {
    assert.equal(plugin.configs.v2.rules[`solid-checker/${obligation}`], undefined);
    assert.ok(!v2.rules.some(entry => entry.name === obligation));
  }
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
  // One pass: its rules share an analysis when their settings normalize to
  // the same identity.
  const pass = { sourceCode: sourceCode("") };
  const context = (preset, enableRule) => Object.assign(Object.create(pass), {
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
  const pass = { sourceCode: sourceCode("") };
  const context = runtime => Object.assign(Object.create(pass), {
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

test("the rules enabled for a file are forwarded so native skips the comments ESLint applies (ADR 0429)", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-host-suppresses-"));
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
    settings: { solidChecker: { command: process.execPath, commandArgs: [analyzer, calls], project } },
    options: [],
    sourceCode: sourceCode("export {};\n"),
    report() {}
  };
  const flagged = () => {
    const args = JSON.parse(readFileSync(calls, "utf8"));
    return args.flatMap((arg, index) => (arg === "--host-suppresses" ? [args[index + 1]] : []));
  };
  try {
    // A member reported through `certification` is not ESLint's to suppress.
    plugin._testing.snapshotCache.clear();
    const certification = plugin.rules.certification.create(context);
    certification.Program({ type: "Program" });
    certification["Program:exit"]();
    assert.deepEqual(flagged(), []);

    // Enabled per-rule rules, a deprecated key and the no-op group key: ESLint
    // matches a comment against the ids that reported, so those that report
    // are forwarded and `reactivity` (which never reports) is not.
    const created = ["strict-read-untracked", "no-direct-mutation", "reactivity"].map(
      name => plugin.rules[name].create(context)
    );
    created[0].Program({ type: "Program" });
    assert.deepEqual(flagged(), ["no-direct-mutation", "strict-read-untracked"]);
    for (const listeners of created) listeners["Program:exit"]?.();

    // Released with the pass: the next one starts clean.
    plugin._testing.snapshotCache.clear();
    const next = plugin.rules.certification.create(context);
    next.Program({ type: "Program" });
    next["Program:exit"]();
    assert.deepEqual(flagged(), []);
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

test("ESLint applies a disable comment to the per-rule id that reported (ADR 0429)", () => {
  const finding = (rule, code, start, end) => ({
    id: code, rule, kind: "violation", severity: "error", message: `${rule} here`,
    primaryLocation: { path: "App.js", startByte: start, endByte: end, line: 2, column: 1 }
  });
  const source = "// eslint-disable-next-line solid-checker/strict-read-untracked\nvalue;\nother;\n";
  const at = needle => source.indexOf(needle);
  const verify = rules => new Linter().verify(
    source,
    [{
      plugins: { "solid-checker": plugin },
      linterOptions: { reportUnusedDisableDirectives: "error" },
      settings: { solidChecker: { snapshot: { status: "violation", findings: [
        finding("strict-read-untracked", "SC1001", at("value"), at("value") + 5)
      ] } } },
      rules
    }],
    { filename: "App.js" }
  );
  // Reported under its own id: ESLint suppresses it, and the directive is used.
  assert.deepEqual(verify({ "solid-checker/strict-read-untracked": "error" }), []);
  // Reported through `certification`, the comment does not match what ESLint
  // sees: native is the one that applies it (and then reports nothing).
  const unmatched = verify({ "solid-checker/certification": "error" });
  assert.deepEqual(
    unmatched.map(message => message.ruleId).sort(),
    [null, "solid-checker/certification"]
  );
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

test("every non-v1 RULE_ALIASES row is a deprecated ESLint key (dialect.rs is the authority)", () => {
  const source = readFileSync(new URL("../../../rust/crates/solid-facts-backend/src/dialect.rs", import.meta.url), "utf8");
  const table = source.slice(
    source.indexOf("pub const RULE_ALIASES"),
    source.indexOf("pub const CODE_SCOPED_ALIASES")
  );
  const rows = [...table.matchAll(/\(\s*"([^"]+)",\s*"([^"]+)"\s*,?\s*\)/g)]
    .map(([, oldName, currentName]) => [oldName, currentName])
    .filter(([oldName]) => !oldName.startsWith("v1/"));
  assert.ok(rows.length >= 14, `parsed ${rows.length} rows`);
  const keys = new Map(plugin._testing.deprecatedRuleKeys.map(([oldName, currentName]) => [oldName, currentName]));
  for (const [oldName, currentName] of rows) {
    assert.equal(keys.get(oldName), currentName, `deprecated key ${oldName}`);
  }
  assert.equal(keys.size, rows.length, "no ESLint key lacks a native alias");
});

test("every former name of an obligation in RETIRED_RULES is a retired no-op ESLint key (ADR 0440)", () => {
  const source = readFileSync(new URL("../../../rust/crates/solid-facts-backend/src/dialect.rs", import.meta.url), "utf8");
  const table = source.slice(
    source.indexOf("pub const RETIRED_RULES"),
    source.indexOf("pub const RULE_ALIASES")
  );
  const names = [...table.matchAll(/\(\s*"([^"]+)",\s*(?:RETIRED_AS_OBLIGATION_\w+|"[^"]*obligation and not a rule[^"]*")\s*,?\s*\)/g)]
    .map(([, name]) => name)
    .filter(name => !name.startsWith("v1/"));
  assert.equal(names.length, 7, `parsed ${names.join()}`);
  assert.deepEqual([...names].sort(), [...plugin._testing.retiredRuleKeys].sort());
});

test("the reactivity group key is valid in a config and in a disable comment, and reports nothing (ADR 0428)", () => {
  assert.ok(plugin.rules.reactivity);
  assert.deepEqual(plugin.rules.reactivity.create({}), {});
  const linter = new Linter();
  const source = "// eslint-disable-next-line solid-checker/reactivity\nconst answer = 42;\n";
  const messages = linter.verify(
    source,
    [{
      plugins: { "solid-checker": plugin },
      linterOptions: { reportUnusedDisableDirectives: "off" },
      settings: { solidChecker: { snapshot: { status: "certified", findings: [] } } },
      rules: { "solid-checker/reactivity": "error", "solid-checker/no-owner-cleanup": "error" }
    }],
    { filename: "App.js" }
  );
  assert.deepEqual(messages, []);
  // The group is not a finding's rule, so no manifest entry may take its name.
  for (const catalog of Object.values(plugin._testing.manifests)) {
    assert.ok(!catalog.rules.some(entry => entry.name === "reactivity"));
  }
});

test("a former name of a merged rule reports and owns its own code only (ADR 0426)", () => {
  const merged = "no-await-primitive-in-tracked-scope";
  const findings = [
    finding("SC2004", merged, 0, 2),
    finding("SC2005", merged, 3, 5)
  ];
  const ids = reported => reported.map(entry => entry.data.message.slice(1, 7)).sort();
  for (const [rules, expected] of [
    [["resolve-in-tracked-scope"], ["SC2004"]],
    [["resolve-in-reactive-scope"], ["SC2004"]],
    [["until-in-tracked-scope"], ["SC2005"]],
    [["resolve-in-tracked-scope", "until-in-tracked-scope"], ["SC2004", "SC2005"]],
    [[merged], ["SC2004", "SC2005"]]
  ]) {
    const reported = [];
    lintPass(rules, syntheticContext({ findings }, reported));
    assert.deepEqual(ids(reported), expected, rules.join());
  }
  // Certification skips only the code the enabled old key owns, so the other
  // code is neither dropped nor reported twice.
  for (const [rules, expected] of [
    [["certification", "resolve-in-tracked-scope"], ["SC2004", "SC2005"]],
    [["certification", "until-in-tracked-scope"], ["SC2004", "SC2005"]],
    [["certification", merged], ["SC2004", "SC2005"]]
  ]) {
    const reported = [];
    lintPass(rules, syntheticContext({ findings }, reported));
    assert.deepEqual(ids(reported), expected, rules.join());
  }
  assert.equal(plugin._testing.ownedRules.size, 0);
});

test("the keys of SC9005, SC9011 and SC9012 still load and do nothing (ADR 0440)", () => {
  for (const name of [
    "package-behavior-unavailable",
    "package-contract-incomplete",
    "package-contract-export-missing",
    "package-contract-missing",
    "package-contract-callback-missing",
    "reactive-dispatch-unresolved",
    "reactive-source-uncaptured"
  ]) {
    assert.ok(plugin._testing.retiredRuleKeys.includes(name), name);
    const retired = plugin.rules[name];
    assert.ok(retired, name);
    assert.equal(retired.meta.deprecated, true);
    assert.deepEqual(retired.meta.replacedBy, []);
    // It registers nothing and reports nothing, whatever the snapshot holds.
    assert.deepEqual(retired.create({}), {});
  }
  assert.ok(
    !plugin._testing.deprecatedRuleKeys.some(([oldName]) => oldName === "package-contract-incomplete"),
    "it no longer delegates to a rule"
  );
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
  // A finding of a package subject (SC9005's shape, though SC9005 is an
  // internal obligation and has no rule of its own, ADR 0440) is collapsed over
  // the whole project: its related locations are further sites, not context.
  // A per-file reporter must still see it in each file that has one, once, at
  // that file's first site.
  // A violation, because uncertifiable findings are internal and never
  // reported (`internal findings are never reported` below); the collapse
  // depends on the subject kind alone.
  const collapsed = {
    id: "SC1001",
    rule: "strict-read-untracked",
    kind: "violation",
    severity: "warning",
    message: "pkg is not analyzed from source here, so the behaviour of imported export run is unknown; " +
      "code flowing through it cannot be certified (3 call sites)",
    analysisContext: "callbacks",
    subjectKind: "package-export",
    primaryLocation: { path: "/tmp/app/App.ts", startByte: 2, endByte: 4 },
    relatedLocations: [
      { path: "/tmp/app/Other.ts", startByte: 8, endByte: 10 },
      { path: "/tmp/app/Other.ts", startByte: 4, endByte: 6 }
    ]
  };
  const snapshot = { status: "uncertifiable", findings: [collapsed] };
  const text = "0123456789ab";
  const rule = "strict-read-untracked";

  // The per-rule rule reports it; the catch-all leaves findings a
  // per-rule rule owns to that rule.

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

test("internal findings are never reported, project notices are", () => {
  const text = "0123456789ab";
  const internal = {
    id: "SC1001",
    rule: "strict-read-untracked",
    kind: "uncertifiable",
    severity: "warning",
    message: "not proven either way",
    subjectKind: "accessor",
    primaryLocation: { path: "/tmp/app/App.ts", startByte: 2, endByte: 4 }
  };
  const notice = {
    ...internal,
    id: "SC9014",
    rule: "unaudited-solid-release",
    message: "unaudited release",
    subjectKind: "project",
    primaryLocation: { path: "/tmp/app/node_modules/solid-js/package.json", startByte: 0, endByte: 1 }
  };
  const snapshot = { status: "uncertifiable", findings: [internal, notice] };
  assert.equal(run(snapshot, "/tmp/app/App.ts", text, "strict-read-untracked").length, 0);
  const all = run(snapshot, "/tmp/app/App.ts", text);
  assert.equal(all.length, 1);
  assert.match(all[0].data.message, /^\[SC9014\] unaudited release/);
});

test("a proven compiler refusal is reported in its own file, an unproven one in every file (ADR 0441)", () => {
  const text = "0123456789ab";
  const refusal = {
    id: "SC9015",
    rule: "compiler-refused-source",
    kind: "violation",
    severity: "error",
    message: "the pinned Solid 2 compiler refuses /tmp/app/Refused.tsx",
    subjectKind: "file",
    primaryLocation: { path: "/tmp/app/Refused.tsx", startByte: 0, endByte: 0 }
  };
  const snapshot = { status: "violation", findings: [refusal] };
  const own = run(snapshot, "/tmp/app/Refused.tsx", text, "compiler-refused-source");
  assert.equal(own.length, 1);
  assert.match(own[0].data.message, /^\[SC9015\] the pinned Solid 2 compiler refuses/);
  assert.equal(run(snapshot, "/tmp/app/Other.tsx", text, "compiler-refused-source").length, 0);

  const notice = {
    ...refusal,
    kind: "uncertifiable",
    severity: "warning",
    subjectKind: "project",
    message: "the Solid compiler refused /tmp/app/Refused.tsx"
  };
  const noticed = { status: "uncertifiable", findings: [notice] };
  assert.equal(run(noticed, "/tmp/app/Other.tsx", text, "compiler-refused-source").length, 1);
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

test("settings retired with package-contract consumption are refused, not ignored", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-retired-"));
  const project = join(root, "tsconfig.json");
  writeFileSync(project, "{}\n");
  const { command, commandArgs, invocations } = recordingAnalyzer(root);
  plugin._testing.snapshotCache.clear();
  for (const [name, value] of [
    ["acceptedContracts", "catalog.json"],
    ["receiptTrustConfiguration", "trust.json"],
    ["dependencySource", false]
  ]) {
    assert.throws(
      () => plugin._testing.loadSnapshot({
        filename: join(root, "App.tsx"),
        physicalFilename: join(root, "App.tsx"),
        settings: { solidChecker: { command, commandArgs, project, [name]: value } },
        options: []
      }),
      new RegExp(`settings\\.solidChecker\\.${name} was removed`)
    );
  }
  assert.equal(invocations().length, 0, "the checker never starts");
  plugin._testing.snapshotCache.clear();
});

test("a note reaches ESLint once per linted file, as a warning", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-note-"));
  const project = join(root, "tsconfig.json");
  writeFileSync(project, "{}\n");
  // The native checker's notice, in shape: exit 0, findings on stdout, the
  // note on stderr, which the adapter used to drop.
  const note =
    "solid-checker: note: packages are not analyzed from source by default: no Solid Vite " +
    "plugin resolves from the project; their behaviour stays uncertifiable";
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
    assert.match(messages[0].message, /^\[solid-checker note\] packages are not analyzed/);
    assert.ok(!messages[0].message.includes("timing noise"), "only note lines surface");
  }
  assert.equal(invocations().length, 2, "each linted file's pass asks native once");

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
  // Ten passes so far (2 files, 4 configs, 4 single lints): one analysis each,
  // shared by the note rule and every other rule of the pass.
  assert.equal(invocations().length, 10, "the note rule shares the pass's analysis");

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

test("every run is reused within one file's pass only", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-source-"));
  try {
    const project = join(root, "tsconfig.json");
    writeFileSync(project, "{}\n");
    const { command, commandArgs, invocations } = recordingAnalyzer(root);
    const context = (settings, extraArgs = []) => ({
      filename: join(root, "App.tsx"),
      physicalFilename: join(root, "App.tsx"),
      sourceCode: sourceCode("export {};"),
      settings: { solidChecker: {
        command, commandArgs: [...commandArgs, ...extraArgs], project, ...settings
      } },
      options: []
    });
    const host = { runtime: { target: "browser", build: "development" } };
    plugin._testing.snapshotCache.clear();

    // Under a declared target and build: one analysis per pass, none reused
    // across passes, and the native default is left alone.
    const first = context(host);
    plugin._testing.loadSnapshot(first);
    plugin._testing.loadSnapshot(Object.create(first));
    assert.equal(invocations().length, 1, "every rule of one pass shares its analysis");
    plugin._testing.loadSnapshot(context(host));
    assert.equal(invocations().length, 2, "a later pass analyzes again");
    assert.equal(plugin._testing.snapshotCache.size, 0, "nothing enters the cross-pass cache");

    // So is every other run: no cross-pass cache follows the inputs.
    plugin._testing.loadSnapshot(context({ runtime: { target: "browser" } }));
    plugin._testing.loadSnapshot(context({ runtime: { target: "browser" } }));
    assert.equal(invocations().length, 4);
    assert.equal(plugin._testing.snapshotCache.size, 0);
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

// Review round 1 of ADR 0297: an analyzer whose snapshot names the epoch on
// disk at the time it ran, standing in for package or project bytes native
// reads and the linted file does not show.
function epochAnalyzer(root) {
  const epoch = join(root, "epoch.txt");
  const runs = join(root, "runs.txt");
  const analyzer = join(root, "epoch-analyzer.mjs");
  writeFileSync(epoch, "before-edit");
  writeFileSync(analyzer, `import { appendFileSync, readFileSync } from "node:fs";
appendFileSync(${JSON.stringify(runs)}, "x");
process.stdout.write(JSON.stringify({ status: "certified", findings: [],
  epoch: readFileSync(${JSON.stringify(epoch)}, "utf8"),
  run: readFileSync(${JSON.stringify(runs)}, "utf8").length }));
`);
  return {
    command: process.execPath,
    commandArgs: [analyzer],
    edit: () => writeFileSync(epoch, "after-edit"),
    runs: () => {
      try {
        return readFileSync(runs, "utf8").length;
      } catch {
        return 0;
      }
    }
  };
}

// Nothing is reused across passes (native answers a repeat check from its
// daemon), while the rules of one pass still share one analysis. The same token
// consumed as a value option's value is a value, and changes nothing.
test("a run with a native flag is reused within one pass only", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-native-flag-"));
  try {
    const project = join(root, "tsconfig.json");
    writeFileSync(project, "{}\n");
    const { command, commandArgs, edit, runs } = epochAnalyzer(root);
    const observe = {
      create(context) {
        return {
          Program(node) {
            const snapshot = plugin._testing.loadSnapshot(context);
            context.report({ node, message: `${snapshot.epoch} ${snapshot.run}` });
          }
        };
      }
    };
    const linter = new Linter({ cwd: root });
    const messages = (extraArgs, settings = {}) => linter.verify("export {};", [{
      plugins: { review: { rules: { observe, other: observe } }, "solid-checker": plugin },
      settings: { solidChecker: {
        project, command, commandArgs: [...commandArgs, ...extraArgs], ...settings
      } },
      rules: {
        "review/observe": "warn",
        "review/other": "warn",
        "solid-checker/certification": "error"
      }
    }], { filename: join(root, "App.js") }).map(m => m.message);
    plugin._testing.snapshotCache.clear();

    const off = ["--internal"];
    assert.deepEqual(messages(off), ["before-edit 1", "before-edit 1"],
      "every rule of one pass shares one analysis");
    edit();
    assert.deepEqual(messages(off), ["after-edit 2", "after-edit 2"],
      "the next pass analyzes afresh and sees the changed input");
    assert.deepEqual(messages(off), ["after-edit 3", "after-edit 3"],
      "nor does a third");
    assert.equal(plugin._testing.snapshotCache.size, 0, "nothing enters the cross-pass cache");
    assert.equal(runs(), 3);

    // Any other run, too: a later edit is seen on the next pass.
    const valued = ["--runtime-condition", "--internal"];
    assert.deepEqual(messages(valued), ["after-edit 4", "after-edit 4"]);
    writeFileSync(join(root, "epoch.txt"), "later-edit");
    assert.deepEqual(messages(valued), ["later-edit 5", "later-edit 5"],
      "no cross-pass cache serves a stale snapshot");
    assert.equal(runs(), 5);
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

test("a native flag in commandArgs reaches native unchanged", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-native-flag-argv-"));
  try {
    const project = join(root, "tsconfig.json");
    writeFileSync(project, "{}\n");
    const { command, commandArgs, invocations } = recordingAnalyzer(root);
    plugin._testing.snapshotCache.clear();
    plugin._testing.loadSnapshot({
      filename: join(root, "App.tsx"),
      physicalFilename: join(root, "App.tsx"),
      sourceCode: sourceCode("export {};"),
      settings: { solidChecker: {
        command, commandArgs: [...commandArgs, "--internal"], project
      } },
      options: []
    });
    assert.equal(invocations().length, 1);
    assert.equal(invocations()[0].filter(arg => arg === "--internal").length, 1);
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

test("a SourceCode linted again is a new pass and analyzes again (ADR 0297 review)", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-reverify-"));
  try {
    const project = join(root, "tsconfig.json");
    writeFileSync(project, "{}\n");
    const { command, commandArgs, edit, runs } = epochAnalyzer(root);
    const observe = {
      create(context) {
        return {
          Program(node) {
            const snapshot = plugin._testing.loadSnapshot(context);
            context.report({ node, message: `${snapshot.epoch} ${snapshot.run}` });
          }
        };
      }
    };
    const config = [{
      plugins: { review: { rules: { observe, other: observe } }, "solid-checker": plugin },
      settings: { solidChecker: {
        project, command, commandArgs, runtime: { target: "browser", build: "development" }
      } },
      rules: {
        "review/observe": "warn",
        "review/other": "warn",
        "solid-checker/certification": "error"
      }
    }];
    const linter = new Linter({ cwd: root });
    const messages = (input, configs = config) =>
      linter.verify(input, configs, { filename: join(root, "App.js") }).map(m => m.message);
    assert.deepEqual(messages("export {};"), ["before-edit 1", "before-edit 1"],
      "every rule of one pass shares one analysis");
    const sourceCode = linter.getSourceCode();
    edit();
    assert.deepEqual(messages(sourceCode), ["after-edit 2", "after-edit 2"],
      "the same SourceCode linted again is a new pass");
    assert.deepEqual(messages("export {};"), ["after-edit 3", "after-edit 3"]);
    assert.equal(runs(), 3);

    // Without an adapter rule to end it, the pass is still ESLint's per-pass
    // file context, not the SourceCode.
    const bare = [{ ...config[0], rules: { "review/observe": "warn" } }];
    assert.deepEqual(messages(sourceCode, bare), ["after-edit 4"]);
    assert.equal(plugin._testing.snapshotCache.size, 0);
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

test("no commandArgs make a pass reuse another's analysis", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-adapter-argv-"));
  try {
    const project = join(root, "tsconfig.json");
    writeFileSync(project, "{}\n");
    const { command, commandArgs, edit, runs } = epochAnalyzer(root);
    const lint = (settings, extra) => plugin._testing.loadSnapshot({
      filename: join(root, "App.tsx"),
      physicalFilename: join(root, "App.tsx"),
      sourceCode: sourceCode("export {};"),
      settings: { solidChecker: {
        command, commandArgs: [...commandArgs, ...extra], project, ...settings
      } },
      options: []
    }).epoch;
    const host = { runtime: { target: "browser", build: "development" } };
    plugin._testing.snapshotCache.clear();

    // A condition spelled like a flag is a condition: native defaults to
    // source, so no pass reuses another's analysis.
    const condition = ["--runtime-condition", "--internal"];
    assert.equal(lint(host, condition), "before-edit");
    edit();
    assert.equal(lint(host, condition), "after-edit", "a later pass analyzes again");
    assert.equal(runs(), 2);
    assert.equal(plugin._testing.snapshotCache.size, 0);

    lint({}, ["--some-future-option", "--certify"]);
    lint({}, ["--some-future-option", "--certify"]);
    assert.equal(runs(), 4);
    lint({}, ["--certify"]);
    lint({}, ["--certify"]);
    assert.equal(runs(), 6);
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});

test("the adapter's native option table is native's argv parser", () => {
  const main = readFileSync(
    new URL("../../../rust/crates/solid-facts-backend/src/main.rs", import.meta.url),
    "utf8"
  );
  const start = main.indexOf("fn request_from_args()");
  const body = main.slice(start, main.indexOf("unknown => return Err", start));
  const arms = [...body.matchAll(/\n {12}((?:"-[^"]+"(?: \| )?)+) =>/g)];
  const values = new Set();
  const flags = new Set();
  arms.forEach((arm, index) => {
    const end = index + 1 < arms.length ? arms[index + 1].index : body.length;
    const consumes = /args\s*\.next\(\)/.test(body.slice(arm.index + arm[0].length, end));
    for (const [, name] of arm[1].matchAll(/"([^"]+)"/g)) (consumes ? values : flags).add(name);
  });
  assert.ok(values.size > 15, "the parser was found");
  assert.deepEqual([...plugin._testing.nativeValueOptions].sort(), [...values].sort());
  assert.deepEqual([...plugin._testing.nativeFlags].sort(), [...flags].sort());
});
