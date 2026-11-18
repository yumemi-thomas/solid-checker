// The checker adapter: materializes a case the way scripts/runtime-oracle.mjs does
// (App.tsx, the oracle's main.tsx, the oracle's tsconfig, inside the pinned install so
// its node_modules is an ancestor), runs the native checker as `feedback run` does
// (`--format json --feedback-facts` for the development runtime), and reduces its
// findings to a verdict per site: violation, uncertifiable, or none.
//
// Uncertifiable is an internal result (ADR 0328); here it means "the checker said
// nothing it can prove", which is never evidence either way.
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { createRequire } from "node:module";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { AssumedReportError, refuseAssumed } from "../lib/assumed.mjs";
import { ignoredRules, isProgramModule } from "./codes.mjs";
import { tsconfigFor } from "./typecheck.mjs";

/**
 * The oracle's entry for a case with no behavioural observation: `mainSource([])` of scripts/runtime-oracle.mjs, read from
 * that file so it cannot drift. It renders every exported component and then drives the page with synchronous DOM events;
 * the checker reads that driver (ADR 0435: a synchronous dispatch withholds event-handler owner proofs), so a verdict
 * computed without it is not the verdict Chrome's run sees.
 */
export function oracleMainSource(oracleText = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "../runtime-oracle.mjs"), "utf8"), mount, entered = null) {
  const open = "const mainSource = ";
  const start = oracleText.indexOf(open);
  const end = oracleText.indexOf("\n`;", start);
  if (start < 0 || end < 0) throw new Error("scripts/runtime-oracle.mjs no longer defines mainSource as a template literal");
  // eslint-disable-next-line no-new-func
  // `behaviourProbe` is "" without observations (scripts/runtime-oracle.mjs: `if (!observations.length) return ""`).
  return new Function("behaviourProbe", `return (${oracleText.slice(start + open.length, end + 2)});`)(() => "")([], mount, entered);
}
export const MAIN_SOURCE = oracleMainSource();
/**
 * The entry a case's oracle.json `mount` gives (ADR 0501), the one the oracle writes for it; MAIN_SOURCE without one. ADR 1089: with the case's App.tsx, a mount that names
 * every export the namespace loop would enter (`enteredExports`) gets the entry without the loop, as the oracle writes it, so the checker reads no namespace hand-out the
 * runtime's entry does not have.
 */
export const mainSourceFor = (oracleJson, appText) => {
  const mount = oracleJson ? JSON.parse(oracleJson).mount : undefined;
  return mount ? oracleMainSource(undefined, mount, appText === undefined ? null : enteredExports(appText)) : MAIN_SOURCE;
};

/** ADR 1089: the version of the harness's entry (`mainSourceFor`), recorded beside GENERATOR_VERSION in grid and audit headers: 2 leaves the namespace loop out of a mount that covers every entered export. */
export const ENTRY_VERSION = 2;

/**
 * `SOLID_CHECKER_FUZZ_BOUNDARY=open|closed` (focus.mjs `--boundary`): every program's checker project gets that boundary's manifest, whatever its
 * oracle.json says. The runtime does not read the manifest, so a program's Chrome record is the same under both (the runtime cache is keyed by the
 * case's files, which the override does not change); the type-check cache is keyed by the project's bytes, so it keys by boundary on its own.
 * Unset: each program's own (none: the install's private manifest, a closed program).
 */
export function boundaryOverride(env = process.env) {
  const value = env.SOLID_CHECKER_FUZZ_BOUNDARY;
  if (value === undefined || value === "") return undefined;
  if (value !== "open" && value !== "closed") throw new Error(`SOLID_CHECKER_FUZZ_BOUNDARY takes open or closed, not ${value}`);
  return value;
}

/**
 * `SOLID_CHECKER_FUZZ_ENTRY=named` (focus.mjs `--entry named`, ADR 0990): the CHECKER project's `main.tsx` names the exports it enters (`import { C1 } from "./App"`, one
 * `render(() => <C1 />)` each) instead of `import * as mod` and a loop over the namespace. The loop makes every export of App entered by code the checker cannot see, so no
 * closed-program proof (a caller census, the closed discharge of an owner's entry) is reached in a grid; the named entry lets them apply. The runtime's entry is unchanged
 * (`mainSourceFor` is what Chrome's records were taken with): `namedEntry` enters exactly the exports the loop enters, in the loop's order, with the loop's driver.
 */
export function entryOverride(env = process.env) {
  const value = env.SOLID_CHECKER_FUZZ_ENTRY;
  if (value === undefined || value === "" || value === "namespace") return undefined;
  if (value !== "named") throw new Error(`SOLID_CHECKER_FUZZ_ENTRY takes namespace or named, not ${value}`);
  return value;
}

let typescriptModule = null;
const typescript = () => (typescriptModule ??= createRequire(join(resolve(dirname(fileURLToPath(import.meta.url)), "../../packages/cli"), "package.json"))("typescript"));

/** Whether an expression is a function value (`true`), a literal value that is none (`false`) or neither knowable here (`null`). */
function functionValue(ts, node, locals, seen = new Set()) {
  if (!node) return null;
  if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression?.(node) || ts.isNonNullExpression(node)) return functionValue(ts, node.expression, locals, seen);
  if (ts.isArrowFunction(node) || ts.isFunctionExpression(node) || ts.isClassExpression(node)) return true;
  if (ts.isObjectLiteralExpression(node) || ts.isArrayLiteralExpression(node) || ts.isStringLiteralLike(node) || ts.isNumericLiteral(node) || node.kind === ts.SyntaxKind.TrueKeyword || node.kind === ts.SyntaxKind.FalseKeyword || node.kind === ts.SyntaxKind.NullKeyword) return false;
  // A name another top-level binding holds (`export { a }` over `const a = …` is handled by the caller; `const b = a` follows `a`).
  if (ts.isIdentifier(node) && locals.has(node.text) && !seen.has(node.text)) { seen.add(node.text); return locals.get(node.text)(seen); }
  return null;
}

/**
 * The exports of App.tsx the namespace loop enters (`default` and the capitalised names whose value is a function), sorted as a module namespace's keys are, or `null` when one
 * of the exports cannot be classified from the text (a value a call computes, a re-export, `export *`): such a program keeps the namespace entry.
 */
export function enteredExports(appText) {
  const ts = typescript();
  const source = ts.createSourceFile("App.tsx", appText, ts.ScriptTarget.ES2022, true, ts.ScriptKind.TSX);
  const locals = new Map();
  for (const statement of source.statements) {
    if (ts.isFunctionDeclaration(statement) && statement.name) locals.set(statement.name.text, () => true);
    else if (ts.isClassDeclaration(statement) && statement.name) locals.set(statement.name.text, () => true);
    else if (ts.isVariableStatement(statement)) for (const one of statement.declarationList.declarations) if (ts.isIdentifier(one.name)) locals.set(one.name.text, (seen) => functionValue(ts, one.initializer, locals, seen));
  }
  const isFunction = (name) => (locals.has(name) ? locals.get(name)(new Set([name])) : null);
  const entered = new Map();
  const exportedAs = (name, isFn) => { if (isFn === null) throw new Error(`cannot classify ${name}`); entered.set(name, isFn); };
  try {
    for (const statement of source.statements) {
      const modifiers = ts.canHaveModifiers(statement) ? ts.getModifiers(statement) ?? [] : [];
      const exported = modifiers.some((m) => m.kind === ts.SyntaxKind.ExportKeyword);
      const isDefault = modifiers.some((m) => m.kind === ts.SyntaxKind.DefaultKeyword);
      if (ts.isExportDeclaration(statement)) {
        if (statement.isTypeOnly) continue;
        if (statement.moduleSpecifier || !statement.exportClause || !ts.isNamedExports(statement.exportClause)) throw new Error("re-export");
        for (const specifier of statement.exportClause.elements) if (!specifier.isTypeOnly) exportedAs(specifier.name.text, isFunction((specifier.propertyName ?? specifier.name).text));
      } else if (ts.isExportAssignment(statement)) {
        if (statement.isExportEquals) throw new Error("export =");
        exportedAs("default", functionValue(ts, statement.expression, locals));
      } else if (!exported) continue;
      else if (isDefault && (ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement))) entered.set("default", true);
      else if (ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) entered.set(statement.name.text, true);
      else if (ts.isVariableStatement(statement)) for (const one of statement.declarationList.declarations) { if (!ts.isIdentifier(one.name)) throw new Error("destructured export"); exportedAs(one.name.text, functionValue(ts, one.initializer, locals)); }
      else if (ts.isInterfaceDeclaration(statement) || ts.isTypeAliasDeclaration(statement)) continue;
      else throw new Error("unclassified export");
    }
  } catch { return null; }
  return [...entered].filter(([name, isFn]) => isFn && (name === "default" || /^[A-Z]/.test(name))).map(([name]) => name).sort();
}

/** How many programs `namedEntry` could not derive an entry for (they kept the namespace one). */
export const namedEntryFallbacks = { count: 0 };
process.on("exit", () => { if (namedEntryFallbacks.count) process.stderr.write(`named entry: ${namedEntryFallbacks.count} programs kept the namespace entry (an export that cannot be classified from the text)\n`); });

/**
 * The named entry of a case (see `entryOverride`): the oracle's entry with `import * as mod` and the namespace loop replaced by one `render` per entered export, the mount names
 * first (as the oracle's mount form does) and the others in namespace order; the oracle's event driver is carried over byte for byte. `null` when the exports cannot be classified.
 */
export function namedEntry(oracleJson, appText) {
  const base = mainSourceFor(oracleJson);
  const driver = base.indexOf("// Drive every handler");
  const names = enteredExports(appText);
  if (driver < 0 || names === null) { namedEntryFallbacks.count += 1; return null; }
  const mount = (oracleJson ? JSON.parse(oracleJson).mount : undefined) ?? [];
  const order = [...mount].sort().filter((name) => names.includes(name) || mount.includes(name)).concat(names.filter((name) => !mount.includes(name)));
  const local = (name) => (name === "default" ? "DefaultExport" : name);
  const imports = order.map((name) => (name === "default" ? "default as DefaultExport" : name));
  return `import { render } from "@solidjs/web";
${imports.length ? `import { ${imports.join(", ")} } from "./App";\n` : ""}
// The named entry (scripts/fuzz/checker.mjs \`entryOverride\`): the oracle's entry with each entered export rendered by name, not found through the namespace.
const root = document.getElementById("root")!;
const host = (name: string) => {
  const section = document.createElement("section");
  section.dataset.export = name;
  root.append(section);
  return section;
};
${order.map((name) => `try {
  render(() => <${local(name)} />, host("${name}"));
} catch (error) {
  console.error("render failed", "${name}", error);
  setTimeout(() => { throw error; });
}
`).join("")}
${base.slice(driver)}`;
}

/** The entry a case's checker project gets: the named one under `entryOverride`, the oracle's otherwise. */
export const entryFor = (oracleJson, appText) => (entryOverride() ? namedEntry(oracleJson, appText) ?? mainSourceFor(oracleJson, appText) : mainSourceFor(oracleJson, appText));

/** The manifest a case's oracle.json `programBoundary` asks for, the one the oracle writes (scripts/runtime-oracle.mjs): a publishing one (an entry field) is open, a private one closed; none without it. */
export const boundaryManifest = (oracleJson, id, override = boundaryOverride()) => {
  const boundary = override ?? (oracleJson ? JSON.parse(oracleJson).programBoundary : undefined);
  if (boundary === undefined) return undefined;
  return `${JSON.stringify(boundary === "open" ? { name: id, main: "main.tsx" } : { name: id, private: true }, null, 2)}\n`;
};

/** Writes a case's generated packages (`{ name: { file: text } }`) into `<dir>/node_modules/<name>`: the project's own dependency, never the shared install's. */
export function installPackages(dir, packages = {}) {
  for (const [name, files] of Object.entries(packages)) {
    for (const [file, text] of Object.entries(files)) {
      const target = join(dir, "node_modules", name, file);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, text);
    }
  }
}

/**
 * This process's share of the install's work area. Program ids are derived from the seed and the index alone (`fuzz-<n>`), so two runs
 * with the same seed and different profiles, or two checkouts on one install, name the same ids; one run's `rmSync` and `App.tsx` would
 * then land in the other's directory (ENOTEMPTY, and a program judged on another profile's text). Ids stay as they are, because
 * `compare-runs.mjs`, the baseline cache and every recorded repro find a program by its id; the directories are what is isolated.
 */
export const WORK_NAMESPACE = `run-${process.pid}-${randomBytes(3).toString("hex")}`;
let keepNamespace = false;
const roots = new Set();
process.on("exit", () => { if (!keepNamespace) for (const root of roots) rmSync(root, { recursive: true, force: true }); });

/** `--keep-work`: leave this process's checker directories in place when it exits. */
export const keepWorkNamespace = () => { keepNamespace = true; };

/** Where checker cases live: inside the install (so its node_modules is an ancestor), in this process's own namespace. */
export function checkerWorkDir(install, id) {
  const root = join(resolve(install), ".solid-checker-fuzz", WORK_NAMESPACE);
  roots.add(root);
  return join(root, id);
}

/**
 * Writes App.tsx, main.tsx and tsconfig.json for `id` under the install; returns the directory.
 * `twin: true` writes the instrumented twin (hits.mjs) as App.tsx instead of the clean text.
 */
export function materializeForChecker(built, install, id = built.id, { twin = false } = {}) {
  const dir = checkerWorkDir(install, id);
  rmSync(dir, { recursive: true, force: true });
  for (const [name, text] of Object.entries(checkerProjectFiles(built, { twin }))) {
    mkdirSync(dirname(join(dir, name)), { recursive: true });
    writeFileSync(join(dir, name), text);
  }
  return dir;
}

/** The files `materializeForChecker` writes, by path relative to the project: App.tsx (or the twin), the modules, the entry, the tsconfig, the packages. */
export function checkerProjectFiles(built, { twin = false } = {}) {
  const modules = projectModules(built.files);
  const files = { "App.tsx": twin ? built.files["App.run.txt"] : built.files["App.tsx"] };
  for (const name of modules) files[name] = built.files[name];
  files["main.tsx"] = entryFor(built.files["oracle.json"], files["App.tsx"]);
  files["tsconfig.json"] = tsconfigFor(["main.tsx", "App.tsx", ...modules]);
  const manifest = boundaryManifest(built.files["oracle.json"], built.id);
  if (manifest) files["package.json"] = manifest;
  for (const [name, packageFiles] of Object.entries(built.packages ?? {})) {
    for (const [file, text] of Object.entries(packageFiles)) files[join("node_modules", name, file)] = text;
  }
  return files;
}

/** Copies a case directory (App.tsx etc.) into the install's checker work area. */
export function materializeFromDirectory(caseDir, install, id) {
  const dir = checkerWorkDir(install, id);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  cpSync(join(caseDir, "App.tsx"), join(dir, "App.tsx"));
  const modules = readdirSync(caseDir).filter((name) => isProjectModule(name));
  for (const name of modules) cpSync(join(caseDir, name), join(dir, name));
  if (existsSync(join(caseDir, "node_modules"))) cpSync(join(caseDir, "node_modules"), join(dir, "node_modules"), { recursive: true });
  writeFileSync(join(dir, "main.tsx"), entryFor(existsSync(join(caseDir, "oracle.json")) ? readFileSync(join(caseDir, "oracle.json"), "utf8") : undefined, readFileSync(join(caseDir, "App.tsx"), "utf8")));
  writeFileSync(join(dir, "tsconfig.json"), tsconfigFor(["main.tsx", "App.tsx", ...modules]));
  const manifest = boundaryManifest(existsSync(join(caseDir, "oracle.json")) ? readFileSync(join(caseDir, "oracle.json"), "utf8") : undefined, id);
  if (manifest) writeFileSync(join(dir, "package.json"), manifest);
  return dir;
}

/** A project module besides App.tsx and the entry: a top-level `.ts`/`.tsx` source (not a declaration file). */
const isProjectModule = isProgramModule;
/** The case's project modules (`helpers.ts`), sorted. */
export const projectModules = (files = {}) => Object.keys(files).filter(isProjectModule).sort();

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
/** The runtime resolver the launcher (packages/cli/bin/launcher.mjs) points the native checker at. */
const RESOLVER = join(repo, "packages/cli/scripts/runtime-resolver.mjs");

const CHECKER_ARGS = ["--format", "json", "--feedback-facts", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];
/**
 * The same run as a user's `solid-checker` has: no `--feedback-facts`. Under a declared target and build an ordinary analysis reads the
 * installed Solid packages from their source (ADR 0297; `defaults_to_dependency_sources` excludes the feedback-facts mode that
 * `feedback run` uses), which is what a case that ships a generated package is about.
 */
const ORDINARY_ARGS = ["--format", "json", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];

/** The checker's environment: the caller's, the runtime resolver, and the producer (`typefacts`). */
export const checkerEnv = ({ typefacts, env = process.env } = {}) => ({
  ...env, SOLID_CHECKER_RUNTIME_RESOLVER: env.SOLID_CHECKER_RUNTIME_RESOLVER ?? RESOLVER,
  ...(typefacts ?? env.SOLID_TYPEFACTS_BIN ? { SOLID_TYPEFACTS_BIN: typefacts ?? env.SOLID_TYPEFACTS_BIN } : {}),
});

/**
 * Runs the native checker on a materialized directory.
 * `bin` and `typefacts` set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN (the same variables the oracle needs).
 * `ordinary` runs as a user does (ORDINARY_ARGS), so the dependency packages of the project are read from source.
 * @returns {Promise<{ ok: boolean, findings: object[], wallMs: number, error?: string, exitStatus?: number }>} It rejects with an
 * `AssumedReportError` when the checker printed a what-if report (ADR 0950; see `checkerResult`).
 */
export function runChecker(dir, { bin, typefacts, timeoutMs = 180000, env = process.env, ordinary = false } = {}) {
  return new Promise((done, refuse) => {
    const executable = bin ?? env.SOLID_CHECKER_NATIVE_BIN;
    if (!executable || !existsSync(executable)) { done({ ok: false, findings: [], wallMs: 0, error: `checker binary not found: ${executable}` }); return; }
    const started = Date.now();
    const child = spawn(executable, ["--project", join(dir, "tsconfig.json"), ...(ordinary ? ORDINARY_ARGS : CHECKER_ARGS)], {
      cwd: dir, stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...checkerEnv({ typefacts, env }), SOLID_CHECKER_DAEMON: "0",
      },
    });
    let stdout = ""; let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", (error) => { clearTimeout(timer); done({ ok: false, findings: [], wallMs: Date.now() - started, error: error.message }); });
    child.on("close", (status) => {
      clearTimeout(timer);
      try { done(checkerResult({ status, stdout, stderr, wallMs: Date.now() - started }, dir)); } catch (error) { if (error instanceof AssumedReportError) refuse(error); else throw error; }
    });
  });
}

/**
 * A checker run's result from what it printed (`runChecker`, and the Rust executor's raw answers, exec.mjs): exit 0 and 1 carry one
 * JSON document; anything else, or findings that map to no program file, is a failed run.
 *
 * A what-if report (ADR 0950: a top-level `assumed`, or a finding with an `assumed` premise) is none of those: it is a prediction, so it
 * THROWS an `AssumedReportError` instead of returning `ok: false`. A failed run is one program's harness row, and a caller that does not
 * read `ok` (or counts the row and moves on) would turn the refusal into "no findings"; nothing a tainted run says about any program is a
 * fact about the shipped checker, so the whole run stops.
 */
export function checkerResult({ status, stdout, stderr = "", wallMs = 0, spawnError, warm }, dir) {
  if (spawnError) return { ok: false, findings: [], wallMs, error: spawnError };
  // `--warm-verify` (ADR 0543): the retained daemon's answer must be the one-shot answer at the same path, byte for byte.
  if (warm?.identical === false) return { ok: false, findings: [], wallMs, exitStatus: status, error: `the warm checker's answer differs from a one-shot run at ${dir} (exit ${status} against ${warm.oneShotStatus})` };
  // The oracle reads the native child's stdout as one JSON document; exit 0 and 1 both carry one.
  if (status !== 0 && status !== 1) return { ok: false, findings: [], wallMs, exitStatus: status, error: stderr.slice(-800) || `exit ${status}` };
  try {
    const analysis = refuseAssumed(JSON.parse(stdout), "the fuzz judge (scripts/fuzz/checker.mjs)");
    const veto = entryVeto(analysis, dir);
    const moduleFindings = parseModuleFindings(analysis, dir);
    return { ok: true, findings: parseFindings(analysis, dir), ...(moduleFindings.length ? { moduleFindings } : {}), wallMs, exitStatus: status, ...(veto ? { entryVeto: veto } : {}) };
  } catch (error) {
    if (error instanceof AssumedReportError) throw error;
    if (error instanceof UnmappedFindingsError) unmapped.push(error.message);
    return { ok: false, findings: [], wallMs, exitStatus: status, error: error instanceof UnmappedFindingsError ? error.message : `malformed checker output: ${error.message}` };
  }
}

/**
 * The premise by which the harness's own entry (main.tsx, the oracle's event passes) withholds the checker's event-handler proofs
 * (ADR 0435's dispatch veto), or null. While it does, every write and read in a handler is uncertifiable by construction, whatever the
 * program: before ADR 0477 credited the await continuation, every fuzzed handler was. Recorded per run, so a run can say so.
 */
export function entryVeto(analysis, dir) {
  const entry = `${canonical(dir)}/main.tsx:`;
  return (analysis?.premises ?? []).find((premise) => typeof premise === "string" && premise.includes("withholds") && (premise.includes(entry) || premise.includes(`${resolve(dir)}/main.tsx:`))) ?? null;
}

/** The checker answered with findings and none of them is a file of the program it was asked about (see `parseFindings`). */
export class UnmappedFindingsError extends Error {}

const unmapped = [];
/**
 * Stops a run (the entry points call this, exit 2) in which the checker returned findings that mapped to no program file. Such a run
 * has checked nothing: every program would come out clean, and "no false positives" would be a silent pass.
 */
export function assertFindingsMapped() {
  if (unmapped.length) throw new Error(`invalid run: ${unmapped.length} checker run(s) returned findings that map to no file of the program (first: ${unmapped[0]})`);
}

/** A path with its symlinks resolved (the checker reports real paths; `/tmp` is a link to `/private/tmp` on macOS), through its nearest existing ancestor when the file itself is not there. */
function canonical(path) {
  const absolute = resolve(path);
  try { return realpathSync(absolute); } catch { /* not there: resolve the directory it would be in */ }
  const parent = dirname(absolute);
  return parent === absolute ? absolute : join(canonical(parent), absolute.slice(parent.length + 1));
}

/**
 * The hint of a read with no strict-read window (`STRICT_READ_UNWINDOWED`, rust/dialects/solid-v2/rules/src/lib.rs; ADRs 0443, 0530): a stale snapshot
 * reported on purpose, and the finding says the runtime does not warn. That is a claim: `runtimeSilent` is set only once a control run measures it (window.mjs).
 */
export const NO_RUNTIME_WARNING = "No strict-read window is open";

/** The findings in App.tsx, as the oracle reads them (path, rule, id, kind, line range). Paths are compared by realpath on both sides. */
export function parseFindings(analysis, dir) {
  if (!analysis || !Array.isArray(analysis.findings)) throw new Error("no findings array");
  const root = canonical(dir);
  const prefix = `${root}/`;
  const located = analysis.findings.map((finding) => ({ finding, path: finding.primaryLocation?.path ? canonical(finding.primaryLocation.path) : null }));
  if (located.length && !located.some(({ path }) => path?.startsWith(prefix))) {
    throw new UnmappedFindingsError(`the checker returned ${located.length} finding(s) and none is under ${root} (first: ${located[0].path ?? "no path"})`);
  }
  return located
    .filter(({ path }) => path?.startsWith(prefix) && relative(root, path) === "App.tsx")
    .map(({ finding }) => findingRow(finding, "App.tsx"));
}

const findingRow = (finding, file) => ({
  file, rule: finding.rule, id: finding.id, kind: finding.kind, ...(finding.analysisContext ? { blocker: String(finding.analysisContext).split(":")[0] } : {}),
  line: finding.primaryLocation.line, column: finding.primaryLocation.column, endLine: finding.primaryLocation.endLine ?? finding.primaryLocation.line,
  ...(String(finding.hint ?? "").includes(NO_RUNTIME_WARNING) ? { claimsSilent: true } : {}),
});

/**
 * The findings in the program's other modules (`helpers.ts`, `el.ts`: `projectModules`), each with its `file`. Kept apart from `findings`,
 * which every judge reads as App.tsx's by line: only the matrix judge reads these, and only at a frame Chrome recorded in the same module
 * (matrix.mjs `recordedAt`, ADR 0921). The entry (main.tsx) and the packages are not the program's modules.
 */
export function parseModuleFindings(analysis, dir) {
  const root = canonical(dir);
  const prefix = `${root}/`;
  return (analysis?.findings ?? []).flatMap((finding) => {
    const path = finding.primaryLocation?.path ? canonical(finding.primaryLocation.path) : null;
    const file = path?.startsWith(prefix) ? relative(root, path) : null;
    return file && isProjectModule(file) ? [findingRow(finding, file)] : [];
  });
}

/**
 * A verdict per site from the findings whose line range holds the site's line.
 * A style rule the oracle never holds against the runtime (`ignoredRules`) is left out.
 */
export function siteVerdicts(sites, findings) {
  // `sites` may include the glue (auto-sites); a finding on a plumbing line then has a verdict of its own.
  const relevant = findings.filter((finding) => !ignoredRules.includes(finding.rule));
  return sites.map((site) => {
    const covering = relevant.filter((finding) => finding.line <= site.line && site.line <= finding.endLine);
    const verdict = covering.some((finding) => finding.kind === "violation") ? "violation"
      : covering.length ? "uncertifiable" : "none";
    return { siteId: site.id, line: site.line, kind: site.kind, verdict, findings: covering.map((finding) => ({ ...finding, anchored: finding.line === site.line })) };
  });
}
