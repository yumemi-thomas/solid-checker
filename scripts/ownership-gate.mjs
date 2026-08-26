// Product-owned regression and TypeScript-ownership gate.
//
// Cases are deliberately independent of eslint-plugin-solid. Each expected
// finding states both what solid-checker owns and what TypeScript says about
// the same bytes. See fixtures/ownership-cases/README.md.
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { openGateCache } from "./lib/gate-cache.mjs";
import { oracleCompilerOptions, oracleProject, runOracle } from "./tsc-oracle.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const CASES_PATH = join(ROOT, "fixtures/ownership-cases/cases.json");
const LEDGER_PATH = join(ROOT, "fixtures/ownership-cases/migration-ledger.json");
const WORK_ROOT = join(ROOT, "rust/target/ownership-gate");
const requireRetained = process.argv.includes("--require-retained");
const requireComplete = process.argv.includes("--require-complete");

const stable = (value) => {
  if (Array.isArray(value)) return value.map(stable);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => [key, stable(item)]));
  }
  return value;
};
const normalizedStrings = (value = []) => [...new Set(value)].sort();
const byteLength = (value) => Buffer.byteLength(value, "utf8");
const overlaps = (a, b) => a.start < b.end && b.start < a.end;
const safeName = (id) => id.replace(/[^a-zA-Z0-9._-]+/g, "__");
const dialectShort = (dialect) => dialect === "solid-v1" ? "v1" : dialect === "solid-v2" ? "v2" : null;

const fail = (failures, message) => failures.push(message);

const safeEdits = (fixes) => fixes
  .filter((fix) => fix.applicability === "safe")
  .flatMap((fix) => fix.edits)
  .sort((a, b) => a.location.startByte - b.location.startByte);

const applyFixes = (source, fixes) => {
  const bytes = Buffer.from(source, "utf8");
  const edits = safeEdits(fixes);
  const pieces = [];
  let cursor = 0;
  for (const edit of edits) {
    if (edit.location.startByte < cursor) continue;
    pieces.push(bytes.subarray(cursor, edit.location.startByte), Buffer.from(edit.newText, "utf8"));
    cursor = edit.location.endByte;
  }
  pieces.push(bytes.subarray(cursor));
  return Buffer.concat(pieces).toString("utf8");
};

// Preserve only diagnostics whose complete source range survives every edit.
// Insertions/replacements before the range shift it; an edit touching the
// range invalidates the old diagnostic instead of allowing a same-code error
// newly manufactured inside the replacement.
const remapUnchangedRange = (range, edits) => {
  let delta = 0;
  for (const edit of edits) {
    const start = edit.location.startByte;
    const end = edit.location.endByte;
    if (end <= range.start) {
      delta += byteLength(edit.newText) - (end - start);
      continue;
    }
    if (start >= range.end) break;
    return null;
  }
  return { start: range.start + delta, end: range.end + delta };
};

const locate = (variable, ...candidates) => {
  const value = process.env[variable] ?? candidates.find(existsSync);
  if (!value || !existsSync(value)) {
    console.error(`ownership gate: missing ${variable} binary (tried ${candidates.join(", ")})`);
    process.exit(2);
  }
  return value;
};
const CHECKER = locate("SOLID_CHECKER_BIN", join(ROOT, "rust/target/debug/solid-checker-rust"));
const TYPEFACTS = locate("SOLID_TYPEFACTS_BIN", join(ROOT, "bin/solid-typefacts"));
const ownershipCache = openGateCache({
  gate: "ownership",
  scriptPath: fileURLToPath(import.meta.url),
  binaries: [CHECKER, TYPEFACTS, join(ROOT, "bin/solid-typefacts.buildinfo")],
  trees: [oracleProject("v1").root, oracleProject("v2").root],
});
const ownershipOracleCache = openGateCache({
  gate: "ownership-safe-fix-oracle",
  scriptPath: fileURLToPath(import.meta.url),
  trees: [
    oracleProject("v1").root,
    oracleProject("v2").root,
    join(ROOT, "packages/cli/node_modules/typescript"),
  ],
});

const manifest = JSON.parse(readFileSync(CASES_PATH, "utf8"));
const ledger = JSON.parse(readFileSync(LEDGER_PATH, "utf8"));
const failures = [];

if (manifest.schemaVersion !== 1) fail(failures, "cases.json: schemaVersion must be 1");
if (ledger.schemaVersion !== 1) fail(failures, "migration-ledger.json: schemaVersion must be 1");
if (!Array.isArray(manifest.cases)) fail(failures, "cases.json: cases must be an array");
if (!Array.isArray(ledger.cases)) fail(failures, "migration-ledger.json: cases must be an array");

const ids = new Set();
const paths = new Set();
const caseById = new Map();
const resolved = new Map();

const spanOf = (testCase, spec, label) => {
  const text = testCase.source.text;
  let relative;
  if (typeof spec?.marker === "string") {
    const needle = Buffer.from(spec.marker, "utf8");
    const haystack = Buffer.from(text, "utf8");
    const hits = [];
    let cursor = 0;
    while (cursor <= haystack.length - needle.length) {
      const index = haystack.indexOf(needle, cursor);
      if (index < 0) break;
      hits.push(index);
      cursor = index + Math.max(1, needle.length);
    }
    const occurrence = spec.occurrence ?? (hits.length === 1 ? 1 : null);
    if (!occurrence || occurrence < 1 || occurrence > hits.length) {
      fail(failures, `${label}: marker ${JSON.stringify(spec.marker)} occurs ${hits.length} time(s); provide a valid 1-based occurrence`);
      return null;
    }
    relative = { start: hits[occurrence - 1], end: hits[occurrence - 1] + needle.length };
  } else if (Array.isArray(spec?.textRange) && spec.textRange.length === 2) {
    relative = { start: spec.textRange[0], end: spec.textRange[1] };
    if (!Number.isInteger(relative.start) || !Number.isInteger(relative.end) || relative.start < 0 || relative.start >= relative.end || relative.end > byteLength(text)) {
      fail(failures, `${label}: invalid UTF-8 textRange ${JSON.stringify(spec.textRange)}`);
      return null;
    }
  } else {
    fail(failures, `${label}: span must contain marker or textRange`);
    return null;
  }
  const prefix = byteLength(testCase.source.prelude);
  return { start: prefix + relative.start, end: prefix + relative.end };
};

for (const [index, testCase] of (manifest.cases ?? []).entries()) {
  const label = `case[${index}] ${testCase.id ?? "<missing-id>"}`;
  if (typeof testCase.id !== "string" || !testCase.id) fail(failures, `${label}: id must be non-empty`);
  else if (ids.has(testCase.id)) fail(failures, `${label}: duplicate id`);
  else ids.add(testCase.id);
  const short = dialectShort(testCase.dialect);
  if (!short) fail(failures, `${label}: dialect must be solid-v1 or solid-v2`);
  const extension = testCase.source?.extension;
  if (![".ts", ".tsx"].includes(extension)) fail(failures, `${label}: source.extension must be .ts or .tsx`);
  if (typeof testCase.source?.prelude !== "string" || typeof testCase.source?.text !== "string") fail(failures, `${label}: source prelude/text must be strings`);
  const path = `cases/${safeName(testCase.id)}${extension}`;
  if (paths.has(path)) fail(failures, `${label}: derived path collides at ${path}`);
  paths.add(path);
  const expected = testCase.expect?.findings;
  const absent = testCase.expect?.absent;
  if (!Array.isArray(expected) || !Array.isArray(absent)) fail(failures, `${label}: expect.findings and expect.absent must be arrays`);
  if ((expected?.length ?? 0) === 0 && (absent?.length ?? 0) === 0) fail(failures, `${label}: a negative case must name at least one absent rule or family`);
  const findingSpans = [];
  for (const [findingIndex, expectation] of (expected ?? []).entries()) {
    const findingLabel = `${label} finding[${findingIndex}]`;
    const span = spanOf(testCase, expectation.span, findingLabel);
    findingSpans.push(span);
    const ownership = expectation.typescript?.ownership;
    const count = expectation.count ?? 1;
    if (!Number.isInteger(count) || count < 1) fail(failures, `${findingLabel}: count must be a positive integer`);
    if (ownership === "typescript-owned" && expectation.count !== undefined) fail(failures, `${findingLabel}: TypeScript-owned expectations cannot declare a checker finding count`);
    if (!["checker-only", "typescript-owned", "distinct-claim"].includes(ownership)) fail(failures, `${findingLabel}: invalid TypeScript ownership`);
    if (ownership === "distinct-claim" && !expectation.typescript?.justification?.trim()) fail(failures, `${findingLabel}: distinct-claim requires a non-empty justification`);
    for (const [diagnosticIndex, diagnostic] of (expectation.typescript?.diagnostics ?? []).entries()) {
      if (!/^TS\d+$/.test(diagnostic.code ?? "")) fail(failures, `${findingLabel} diagnostic[${diagnosticIndex}]: code must be TS<number>`);
      spanOf(testCase, diagnostic.span, `${findingLabel} diagnostic[${diagnosticIndex}]`);
    }
    if (ownership === "checker-only" && (expectation.typescript?.diagnostics?.length ?? 0) !== 0) fail(failures, `${findingLabel}: checker-only diagnostics must be empty`);
    if (ownership === "typescript-owned" && (expectation.typescript?.diagnostics?.length ?? 0) === 0) fail(failures, `${findingLabel}: typescript-owned requires diagnostics`);
    if (ownership !== "typescript-owned" && (!expectation.kind || !expectation.severity)) fail(failures, `${findingLabel}: live finding requires kind and severity`);
  }
  resolved.set(testCase.id, { testCase, label, short, path, findingSpans });
  caseById.set(testCase.id, testCase);
}

if ((ledger.cases ?? []).length !== 465) {
  fail(failures, `migration ledger must retain exactly 465 reconciled upstream identities`);
}
const ledgerIds = new Set();
for (const [index, row] of (ledger.cases ?? []).entries()) {
  const label = `ledger[${index}] ${row.upstreamCase ?? "<missing-id>"}`;
  if (ledgerIds.has(row.upstreamCase)) fail(failures, `${label}: duplicate upstreamCase`);
  ledgerIds.add(row.upstreamCase);
  if (!["pending", "migrated", "dropped"].includes(row.disposition)) fail(failures, `${label}: invalid disposition`);
  if (row.disposition === "pending" && [row.movedIn, row.ownershipCaseId, row.reason].some((value) => value !== null)) fail(failures, `${label}: pending completion fields must all be null`);
  if (row.disposition === "migrated" && (!row.movedIn || !row.ownershipCaseId || !caseById.has(row.ownershipCaseId) || row.reason !== null)) fail(failures, `${label}: migrated row needs movedIn and a resolvable ownershipCaseId, with null reason`);
  if (row.disposition === "dropped" && (!row.movedIn || !row.reason?.trim() || row.ownershipCaseId !== null)) fail(failures, `${label}: dropped row needs movedIn and reason, with null ownershipCaseId`);
}
if (requireComplete && ledger.cases.some((row) => row.disposition === "pending")) fail(failures, "migration ledger still contains pending rows");
if (requireRetained) {
  const retained = new Set(["reactivity", "no-destructure", "components-return-once", "jsx-no-duplicate-props", "prefer-classlist", "prefer-for", "prefer-show", "jsx-no-undef"]);
  const pending = ledger.cases.filter((row) => retained.has(row.upstreamCase.split("__")[0]) && row.disposition !== "migrated");
  if (pending.length) fail(failures, `${pending.length} retained-rule ledger rows are not migrated`);
}

if (failures.length) {
  console.error(`ownership gate: ${failures.length} manifest/ledger problem(s)\n${failures.map((item) => `  - ${item}`).join("\n")}`);
  process.exit(1);
}

rmSync(WORK_ROOT, { recursive: true, force: true });
mkdirSync(WORK_ROOT, { recursive: true });
const groups = new Map();
for (const value of resolved.values()) {
  const { testCase } = value;
  const config = {
    dialect: testCase.dialect,
    ruleOptions: stable(testCase.ruleOptions ?? {}),
    presets: normalizedStrings(testCase.presets),
    enableRules: normalizedStrings(testCase.enableRules),
  };
  const key = JSON.stringify(config);
  if (!groups.has(key)) groups.set(key, { config, members: [] });
  groups.get(key).members.push(value);
}

const checkerByCase = new Map();
const oracleByCase = new Map();
let groupIndex = 0;
for (const { config, members } of groups.values()) {
  const short = dialectShort(config.dialect);
  const { root } = oracleProject(short);
  const directory = join(WORK_ROOT, `group-${groupIndex++}`);
  const unit = JSON.stringify({
    config,
    members: members.map(member => ({
      id: member.testCase.id,
      path: member.path,
      source: `${member.testCase.source.prelude}${member.testCase.source.text}`,
    })),
  });
  const cached = await ownershipCache.run([unit], () => {
    mkdirSync(join(directory, "cases"), { recursive: true });
    symlinkSync(join(root, "node_modules"), join(directory, "node_modules"), "dir");
    const fileNames = [];
    const oracleInputs = [];
    for (const member of members) {
      const source = `${member.testCase.source.prelude}${member.testCase.source.text}`;
      writeFileSync(join(directory, member.path), source);
      fileNames.push(member.path);
      oracleInputs.push({ name: member.path, code: source });
    }
    writeFileSync(join(directory, "tsconfig.json"), `${JSON.stringify({ compilerOptions: oracleCompilerOptions(short, true), files: fileNames }, null, 2)}\n`);
    if (Object.keys(config.ruleOptions).length) {
      mkdirSync(join(directory, ".solid-checker"), { recursive: true });
      writeFileSync(join(directory, ".solid-checker/rule-options.json"), `${JSON.stringify({ schemaVersion: 1, rules: config.ruleOptions }, null, 2)}\n`);
    }
    const args = ["--format", "json", "--project", join(directory, "tsconfig.json")];
    for (const preset of config.presets) args.push("--preset", preset);
    for (const rule of config.enableRules) args.push("--enable-rule", rule);
    const output = execFileSync(CHECKER, args, { encoding: "utf8", maxBuffer: 1 << 28, env: { ...process.env, SOLID_TYPEFACTS_BIN: TYPEFACTS } });
    const checkerRows = [];
    for (const finding of JSON.parse(output).findings ?? []) {
      const name = finding.primaryLocation.path.split(/[\\/]/).slice(-2).join("/");
      const member = members.find((candidate) => candidate.path === name);
      if (member) checkerRows.push([member.testCase.id, finding]);
    }
    const oracle = runOracle(short, oracleInputs);
    const oracleRows = [];
    for (const member of members) {
      const diagnostics = [];
      const sourceName = member.path.split("/").at(-1);
      for (const pass of ["strict", "loose"]) {
        for (const diagnostic of oracle.passes[pass]) {
          if (diagnostic.category !== "error" || diagnostic.file !== sourceName) continue;
          diagnostics.push({
            pass,
            code: `TS${diagnostic.code}`,
            start: diagnostic.startByte,
            end: diagnostic.endByte,
            subjectStart: diagnostic.subjectStartByte,
            subjectEnd: diagnostic.subjectEndByte,
          });
        }
      }
      oracleRows.push([member.testCase.id, diagnostics]);
    }
    return { checkerRows, oracleRows };
  });
  for (const [id, finding] of cached.value.checkerRows) {
    if (!checkerByCase.has(id)) checkerByCase.set(id, []);
    checkerByCase.get(id).push(finding);
  }
  for (const [id, diagnostics] of cached.value.oracleRows) oracleByCase.set(id, diagnostics);
}

const results = [];
const safeFixInputs = new Map([["v1", []], ["v2", []]]);
for (const value of resolved.values()) {
  const { testCase, label, findingSpans } = value;
  const actual = checkerByCase.get(testCase.id) ?? [];
  const diagnostics = oracleByCase.get(testCase.id) ?? [];
  const claimed = new Set();
  for (const [index, expectation] of testCase.expect.findings.entries()) {
    const span = findingSpans[index];
    const candidates = actual.map((finding, actualIndex) => ({ finding, actualIndex })).filter(({ finding }) => finding.rule === expectation.rule && finding.id === expectation.code && finding.primaryLocation.startByte === span.start && finding.primaryLocation.endByte === span.end);
    const expectedCount = expectation.count ?? 1;
    if (expectation.typescript.ownership === "typescript-owned") {
      if (candidates.length) fail(failures, `${label} finding[${index}]: TypeScript-owned finding was emitted`);
    } else if (candidates.length !== expectedCount) {
      fail(failures, `${label} finding[${index}]: expected exactly ${expectedCount} ${expectation.rule}/${expectation.code} finding(s) at ${span.start}..${span.end}, got ${candidates.length}`);
    } else {
      candidates.forEach(({ actualIndex }) => claimed.add(actualIndex));
      const mismatched = candidates.find(({ finding }) => finding.kind !== expectation.kind || finding.severity !== expectation.severity);
      if (mismatched) fail(failures, `${label} finding[${index}]: expected every finding to be ${expectation.kind}/${expectation.severity}, got ${mismatched.finding.kind}/${mismatched.finding.severity}`);
      const finding = candidates[0].finding;
      const behavior = expectation.fix?.behavior ?? "none";
      const fixes = finding.fixes ?? [];
      if (behavior === "none" && fixes.length) fail(failures, `${label} finding[${index}]: expected no fix, got ${fixes.length}`);
      if (behavior === "safe" && !fixes.some((fix) => fix.applicability === "safe")) fail(failures, `${label} finding[${index}]: expected a safe fix`);
      if (behavior === "unsafe" && !fixes.some((fix) => fix.applicability !== "safe")) fail(failures, `${label} finding[${index}]: expected an unsafe fix`);
      if (behavior === "safe" && typeof expectation.fix?.text === "string") {
        const source = `${testCase.source.prelude}${testCase.source.text}`;
        const fixed = applyFixes(source, fixes);
        if (fixed !== `${testCase.source.prelude}${expectation.fix.text}`) fail(failures, `${label} finding[${index}]: safe fix output differs from expected text`);
      }
      if (behavior === "safe") {
        const source = `${testCase.source.prelude}${testCase.source.text}`;
        const fixed = applyFixes(source, fixes);
        safeFixInputs.get(value.short).push({
          name: `${safeName(testCase.id)}__finding_${index}${testCase.source.extension}`,
          code: fixed,
          allowedDiagnostics: diagnostics,
          edits: safeEdits(fixes),
        });
      }
    }
    const expectedDiagnostics = expectation.typescript.diagnostics ?? [];
    for (const [diagnosticIndex, expected] of expectedDiagnostics.entries()) {
      const diagnosticSpan = spanOf(testCase, expected.span, `${label} finding[${index}] diagnostic[${diagnosticIndex}]`);
      if (!diagnostics.some((diagnostic) => diagnostic.code === expected.code && diagnostic.start === diagnosticSpan.start && diagnostic.end === diagnosticSpan.end)) fail(failures, `${label} finding[${index}]: missing ${expected.code} at ${diagnosticSpan.start}..${diagnosticSpan.end}`);
    }
    const touching = diagnostics.filter((diagnostic) => overlaps(span, {
      start: diagnostic.subjectStart,
      end: diagnostic.subjectEnd,
    }));
    if (expectation.typescript.ownership === "checker-only" && touching.length) fail(failures, `${label} finding[${index}]: checker-only span overlaps ${touching.map((item) => item.code).join(", ")}`);
    if (expectation.typescript.ownership === "distinct-claim" && !touching.length) fail(failures, `${label} finding[${index}]: distinct-claim has no overlapping TypeScript diagnostic`);
  }
  actual.forEach((finding, index) => {
    const ownStart = byteLength(testCase.source.prelude);
    const ownEnd = ownStart + byteLength(testCase.source.text);
    if (!claimed.has(index) && finding.primaryLocation.startByte < ownEnd && ownStart < finding.primaryLocation.endByte) fail(failures, `${label}: unclaimed ${finding.rule}/${finding.id} at ${finding.primaryLocation.startByte}..${finding.primaryLocation.endByte}`);
  });
  for (const absent of testCase.expect.absent) {
    const matched = actual.filter((finding) => absent.rule ? finding.rule === absent.rule : finding.rule.startsWith(absent.family));
    if (matched.length) fail(failures, `${label}: absent ${absent.rule ?? absent.family} emitted ${matched.length} finding(s)`);
  }
  results.push(`${testCase.id}: ${testCase.expect.findings.length} expected, ${actual.length} emitted`);
}

for (const [short, inputs] of safeFixInputs) {
  if (!inputs.length) continue;
  const oracle = (await ownershipOracleCache.run(
    [JSON.stringify({ short, inputs: inputs.map(({ name, code }) => ({ name, code })) })],
    () => runOracle(short, inputs),
  )).value;
  for (const pass of ["strict", "loose"]) {
    for (const input of inputs) {
      const observed = new Map();
      for (const diagnostic of oracle.passes[pass]) {
        if (diagnostic.category !== "error" || diagnostic.file !== input.name) continue;
        const identity = `TS${diagnostic.code}:${diagnostic.startByte}:${diagnostic.endByte}`;
        observed.set(identity, (observed.get(identity) ?? 0) + 1);
      }
      const allowed = new Map();
      for (const diagnostic of input.allowedDiagnostics) {
        if (diagnostic.pass !== pass) continue;
        const mapped = remapUnchangedRange(
          { start: diagnostic.start, end: diagnostic.end },
          input.edits,
        );
        if (!mapped) continue;
        const identity = `${diagnostic.code}:${mapped.start}:${mapped.end}`;
        allowed.set(identity, (allowed.get(identity) ?? 0) + 1);
      }
      for (const [identity, count] of observed) {
        if (count > (allowed.get(identity) ?? 0)) {
          fail(
            failures,
            `safe fix ${input.name} introduces ${count - (allowed.get(identity) ?? 0)} new ${identity.split(":", 1)[0]} error(s) against the real ${short} published typings in ${pass} mode`,
          );
        }
      }
    }
  }
}

if (failures.length) {
  console.error(`ownership gate: ${failures.length} problem(s)\n${failures.map((item) => `  - ${item}`).join("\n")}`);
  process.exit(1);
}
console.log(`ownership gate: ${results.length} cases passed; ledger ${ledger.cases.length} rows (${ledger.cases.filter((row) => row.disposition === "pending").length} pending); ${ownershipCache.summary()}; safe-fix ${ownershipOracleCache.summary()}`);
