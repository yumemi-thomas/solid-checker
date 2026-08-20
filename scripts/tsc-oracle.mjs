// The `tsc` oracle: answers "does TypeScript already report this?" against the
// *real published* Solid typings.
//
// AGENTS.md's absolute rule -- never report what `tsc` reports -- is only
// enforceable if that question has a reproducible answer. Fixture stubs cannot
// answer it: a stub looser than the real package manufactures a defect no real
// project can produce, and every gate stays green while a rule duplicates
// `tsc`. So this script never reads a fixture's `solid-js.d.ts`. It compiles
// the snippet against packages installed from `fixtures/tsc-oracle/packages.json`
// at the exact versions this repository audits, and refuses to run if the
// installed version is not the audited one.
//
//   node scripts/tsc-oracle.mjs provision [--dialect v1|v2|all]
//   node scripts/tsc-oracle.mjs check --dialect v2 --file a.tsx [--file b.tsx] [--json]
//   node scripts/tsc-oracle.mjs check --dialect v1 --code '<snippet>' [--json]
//   node scripts/tsc-oracle.mjs versions [--json]
//
// Two passes run for every input: `strict` and `loose` (the same options with
// `strict: false`). Reporting them apart is what distinguishes "TypeScript
// covers this" from "TypeScript covers this only under `strict`" -- a
// distinction the absolute rule explicitly refuses to treat as an exception,
// but which a ledger entry still has to state.
//
// Diagnostics carry byte spans, because the gate compares them against a
// checker finding's span, and a line/column pair cannot express "the same
// expression" when one column counts UTF-16 units.
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = join(ROOT, "fixtures/tsc-oracle/packages.json");
// Under rust/target so it shares the repository's one ignored build root; the
// oracle's installs are build products, not fixtures.
const ORACLE_ROOT = join(ROOT, "rust/target/tsc-oracle");

const require = createRequire(join(ROOT, "packages/cli/package.json"));
const ts = require("typescript");

const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));

const dialectSpec = (dialect) => {
  const spec = manifest.dialects[dialect];
  if (!spec) {
    throw new Error(
      `unknown dialect ${dialect}; manifest declares ${Object.keys(manifest.dialects).join(", ")}`,
    );
  }
  return spec;
};

const installedVersion = (root, name) => {
  const manifestPath = join(root, "node_modules", name, "package.json");
  if (!existsSync(manifestPath)) return null;
  return JSON.parse(readFileSync(manifestPath, "utf8")).version ?? null;
};

// Absence and version drift are both hard failures, never a skip. A silently
// skipped oracle is exactly the "green gate that verifies nothing" trap the
// SOLID_TYPEFACTS_BIN canary exists to prevent.
const assertProvisioned = (dialect) => {
  const spec = dialectSpec(dialect);
  const root = join(ORACLE_ROOT, dialect);
  const wrong = [];
  for (const [name, expected] of Object.entries(spec.expect)) {
    const actual = installedVersion(root, name);
    if (actual !== expected) wrong.push(`${name}: expected ${expected}, found ${actual ?? "nothing"}`);
  }
  if (wrong.length) {
    const detail = wrong.map((line) => `  ${line}`).join("\n");
    throw new Error(
      `tsc oracle for ${dialect} is not provisioned at the audited versions:\n${detail}\n` +
        `run: node scripts/tsc-oracle.mjs provision --dialect ${dialect}`,
    );
  }
  return root;
};

const provision = (dialect) => {
  const spec = dialectSpec(dialect);
  const root = join(ORACLE_ROOT, dialect);
  mkdirSync(root, { recursive: true });
  writeFileSync(
    join(root, "package.json"),
    `${JSON.stringify({ name: `solid-checker-tsc-oracle-${dialect}`, private: true, version: "0.0.0" }, null, 2)}\n`,
  );
  execFileSync("npm", ["install", "--no-audit", "--no-fund", "--save-exact", ...spec.install], {
    cwd: root,
    stdio: "inherit",
  });
  assertProvisioned(dialect);
  const versions = Object.fromEntries(
    Object.keys(spec.expect).map((name) => [name, installedVersion(root, name)]),
  );
  console.error(`provisioned ${dialect}: ${JSON.stringify(versions)}`);
  return versions;
};

/**
 * The one serializable compiler configuration used by both `tsc` and the
 * checker side of the gate. Keeping this JSON-shaped prevents two handwritten
 * option lists from drifting (notably `lib`, `baseUrl`, and strictness).
 */
export const oracleCompilerOptions = (dialect, strict, overrides = {}) => ({
  target: "ES2022",
  module: "ESNext",
  moduleResolution: "Bundler",
  jsx: "preserve",
  jsxImportSource: dialectSpec(dialect).jsxImportSource,
  lib: ["ES2022", "DOM"],
  // A stray `@types/*` in the install tree must not change the answer.
  types: [],
  skipLibCheck: true,
  allowJs: false,
  baseUrl: ".",
  ...overrides,
  // A case may vary emit semantics, but never the two invariants the oracle
  // compares independently.
  strict,
  noEmit: true,
});

const compilerOptions = (dialect, root, strict, overrides) => {
  const converted = ts.convertCompilerOptionsFromJson(
    oracleCompilerOptions(dialect, strict, overrides),
    root,
  );
  if (converted.errors.length) {
    throw new Error(
      converted.errors.map((error) => ts.flattenDiagnosticMessageText(error.messageText, " ")).join("; "),
    );
  }
  return converted.options;
};

const byteSpan = (text, start, end) => ({
  startByte: Buffer.byteLength(text.slice(0, start), "utf8"),
  endByte: Buffer.byteLength(text.slice(0, end), "utf8"),
});

const isSubject = (node) =>
  ts.isCallExpression(node) ||
  ts.isNewExpression(node) ||
  ts.isJsxAttribute(node) ||
  ts.isJsxElement(node) ||
  ts.isJsxSelfClosingElement(node) ||
  ts.isReturnStatement(node) ||
  ts.isVariableDeclaration(node);

const enclosingSubject = (source, start, end) => {
  let best = source;
  const visit = (node) => {
    const nodeStart = node.getStart(source, false);
    if (nodeStart > start || node.end < end) return;
    if (isSubject(node) && node.end - nodeStart < best.end - best.getStart(source, false)) best = node;
    ts.forEachChild(node, visit);
  };
  visit(source);
  const text = source.getFullText();
  return byteSpan(text, best.getStart(source, false), best.end);
};

const characterOffsetAtByte = (text, byte) => {
  let low = 0;
  let high = text.length;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (Buffer.byteLength(text.slice(0, middle), "utf8") <= byte) low = middle;
    else high = middle - 1;
  }
  return low;
};

/** Return the smallest semantic source subject containing a byte range. */
export const oracleSubjectSpan = (code, startByte, endByte, name = "case.tsx") => {
  const source = ts.createSourceFile(
    name,
    code,
    ts.ScriptTarget.Latest,
    true,
    ts.getScriptKindFromFileName(name),
  );
  return enclosingSubject(
    source,
    characterOffsetAtByte(code, startByte),
    characterOffsetAtByte(code, endByte),
  );
};

// The mapping a ledger entry needs: what the checker claims about a span
// versus what `tsc` claims about it. A diagnostic is recorded with its exact
// byte span so the Phase 4 gate can ask "is there a `tsc` error at this
// finding's span" without guessing at encoding.
const collect = (program, sources) => {
  const out = [];
  for (const file of sources) {
    const source = program.getSourceFile(file);
    if (!source) continue;
    const diagnostics = [
      ...program.getSyntacticDiagnostics(source),
      ...program.getSemanticDiagnostics(source),
    ];
    for (const diagnostic of diagnostics) {
      const start = diagnostic.start ?? 0;
      const { line, character } = source.getLineAndCharacterOfPosition(start);
      const text = source.getFullText();
      const subject = enclosingSubject(source, start, start + (diagnostic.length ?? 0));
      out.push({
        code: diagnostic.code,
        category: ts.DiagnosticCategory[diagnostic.category].toLowerCase(),
        message: ts.flattenDiagnosticMessageText(diagnostic.messageText, " "),
        file: basename(file),
        line: line + 1,
        column: character + 1,
        // Byte offsets, so a non-ASCII case cannot slip -- the same reason
        // Checker fixes use UTF-8 byte offsets and are applied to a Buffer.
        startByte: Buffer.byteLength(text.slice(0, start), "utf8"),
        endByte: Buffer.byteLength(text.slice(0, start + (diagnostic.length ?? 0)), "utf8"),
        subjectStartByte: subject.startByte,
        subjectEndByte: subject.endByte,
        text: text.slice(start, start + (diagnostic.length ?? 0)),
      });
    }
  }
  return out.sort((a, b) => a.startByte - b.startByte || a.code - b.code);
};

/**
 * The audited install for `dialect`, verified to be at the expected versions.
 *
 * Exported so a consumer that needs a *project* rather than a compilation --
 * the oracle gate runs the checker over the same snippets -- resolves against
 * the same tree, behind the same version check, instead of reimplementing it
 * and drifting.
 *
 * @param {"v1"|"v2"} dialect
 * @returns {{root: string, jsxImportSource: string}}
 */
export const oracleProject = (dialect) => ({
  root: assertProvisioned(dialect),
  jsxImportSource: dialectSpec(dialect).jsxImportSource,
});

/**
 * Compile `inputs` against the real typings for `dialect`.
 *
 * @param {"v1"|"v2"} dialect
 * @param {{name: string, code: string}[]} inputs
 * @returns diagnostics from a `strict` and a `loose` pass, plus the versions
 *          they were produced against.
 */
export const runOracle = (dialect, inputs, compilerOptionOverrides = {}) => {
  const spec = dialectSpec(dialect);
  const root = assertProvisioned(dialect);
  // An isolated temp directory: the audited install is never mutated, and the
  // snippet under test never joins a project that could supply extra types.
  const work = mkdtempSync(join(tmpdir(), `solid-tsc-oracle-${dialect}-`));
  try {
    // Node's resolver walks up for node_modules, so linking the audited tree
    // next to the sources is enough -- and keeps the install read-only.
    symlinkSync(join(root, "node_modules"), join(work, "node_modules"), "dir");
    const files = inputs.map(({ name, code }) => {
      const target = join(work, name);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, code);
      return target;
    });
    const passes = {};
    for (const [pass, strict] of [
      ["strict", true],
      ["loose", false],
    ]) {
      const program = ts.createProgram(
        files,
        compilerOptions(dialect, work, strict, compilerOptionOverrides),
      );
      passes[pass] = collect(program, files);
    }
    return {
      dialect,
      typescript: ts.version,
      versions: Object.fromEntries(
        Object.keys(spec.expect).map((name) => [name, installedVersion(root, name)]),
      ),
      inputs: inputs.map(({ name }) => name),
      passes,
    };
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
};

const usage = () => {
  console.error(
    [
      "usage:",
      "  node scripts/tsc-oracle.mjs provision [--dialect v1|v2|all]",
      "  node scripts/tsc-oracle.mjs check --dialect v1|v2 (--file <path>... | --code <snippet>) [--json]",
      "  node scripts/tsc-oracle.mjs versions [--json]",
    ].join("\n"),
  );
  process.exit(2);
};

const parseArgs = (argv) => {
  const options = { dialect: null, files: [], code: null, json: false, name: null };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") options.json = true;
    else if (arg === "--dialect") options.dialect = argv[++index];
    else if (arg === "--file") options.files.push(argv[++index]);
    else if (arg === "--code") options.code = argv[++index];
    else if (arg === "--name") options.name = argv[++index];
    else usage();
  }
  return options;
};

const main = () => {
  const [command, ...rest] = process.argv.slice(2);
  const options = parseArgs(rest);
  if (command === "provision") {
    const dialects =
      !options.dialect || options.dialect === "all" ? Object.keys(manifest.dialects) : [options.dialect];
    for (const dialect of dialects) provision(dialect);
    return;
  }
  if (command === "versions") {
    const report = Object.fromEntries(
      Object.keys(manifest.dialects).map((dialect) => {
        const spec = dialectSpec(dialect);
        const root = join(ORACLE_ROOT, dialect);
        return [
          dialect,
          Object.fromEntries(
            Object.keys(spec.expect).map((name) => [
              name,
              { expected: spec.expect[name], installed: installedVersion(root, name) },
            ]),
          ),
        ];
      }),
    );
    console.log(JSON.stringify({ typescript: ts.version, dialects: report }, null, 2));
    return;
  }
  if (command !== "check") usage();
  if (!options.dialect) usage();
  const inputs = options.code
    ? [{ name: options.name ?? "oracle.tsx", code: options.code.endsWith("\n") ? options.code : `${options.code}\n` }]
    : options.files.map((path) => ({
        name: basename(path),
        code: readFileSync(resolve(path), "utf8"),
      }));
  if (!inputs.length) usage();
  const result = runOracle(options.dialect, inputs);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }
  console.log(`dialect ${result.dialect}  typescript ${result.typescript}`);
  console.log(`packages ${JSON.stringify(result.versions)}`);
  for (const pass of ["strict", "loose"]) {
    const diagnostics = result.passes[pass];
    console.log(`\n--- ${pass}: ${diagnostics.length} diagnostic(s) ---`);
    for (const d of diagnostics) {
      console.log(
        `${d.file}(${d.line},${d.column}) [${d.startByte}..${d.endByte}] TS${d.code}: ${d.message}`,
      );
    }
  }
};

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    main();
  } catch (error) {
    console.error(String(error.message ?? error));
    process.exit(1);
  }
}
