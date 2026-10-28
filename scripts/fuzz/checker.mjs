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
import { cpSync, existsSync, mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { ignoredRules } from "./codes.mjs";
import { tsconfigFor } from "./typecheck.mjs";

/**
 * The oracle's entry for a case with no behavioural observation: `mainSource([])` of scripts/runtime-oracle.mjs, read from
 * that file so it cannot drift. It renders every exported component and then drives the page with synchronous DOM events;
 * the checker reads that driver (ADR 0435: a synchronous dispatch withholds event-handler owner proofs), so a verdict
 * computed without it is not the verdict Chrome's run sees.
 */
export function oracleMainSource(oracleText = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "../runtime-oracle.mjs"), "utf8")) {
  const open = "const mainSource = ";
  const start = oracleText.indexOf(open);
  const end = oracleText.indexOf("\n`;", start);
  if (start < 0 || end < 0) throw new Error("scripts/runtime-oracle.mjs no longer defines mainSource as a template literal");
  // eslint-disable-next-line no-new-func
  // `behaviourProbe` is "" without observations (scripts/runtime-oracle.mjs: `if (!observations.length) return ""`).
  return new Function("behaviourProbe", `return (${oracleText.slice(start + open.length, end + 2)});`)(() => "")([]);
}
export const MAIN_SOURCE = oracleMainSource();

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
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "App.tsx"), twin ? built.files["App.run.txt"] : built.files["App.tsx"]);
  writeFileSync(join(dir, "main.tsx"), MAIN_SOURCE);
  writeFileSync(join(dir, "tsconfig.json"), tsconfigFor(["main.tsx", "App.tsx"]));
  installPackages(dir, built.packages);
  return dir;
}

/** Copies a case directory (App.tsx etc.) into the install's checker work area. */
export function materializeFromDirectory(caseDir, install, id) {
  const dir = checkerWorkDir(install, id);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  cpSync(join(caseDir, "App.tsx"), join(dir, "App.tsx"));
  if (existsSync(join(caseDir, "node_modules"))) cpSync(join(caseDir, "node_modules"), join(dir, "node_modules"), { recursive: true });
  writeFileSync(join(dir, "main.tsx"), MAIN_SOURCE);
  writeFileSync(join(dir, "tsconfig.json"), tsconfigFor(["main.tsx", "App.tsx"]));
  return dir;
}

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

/**
 * Runs the native checker on a materialized directory.
 * `bin` and `typefacts` set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN (the same variables the oracle needs).
 * `ordinary` runs as a user does (ORDINARY_ARGS), so the dependency packages of the project are read from source.
 * @returns {Promise<{ ok: boolean, findings: object[], wallMs: number, error?: string, exitStatus?: number }>}
 */
export function runChecker(dir, { bin, typefacts, timeoutMs = 180000, env = process.env, ordinary = false } = {}) {
  return new Promise((done) => {
    const executable = bin ?? env.SOLID_CHECKER_NATIVE_BIN;
    if (!executable || !existsSync(executable)) { done({ ok: false, findings: [], wallMs: 0, error: `checker binary not found: ${executable}` }); return; }
    const started = Date.now();
    const child = spawn(executable, ["--project", join(dir, "tsconfig.json"), ...(ordinary ? ORDINARY_ARGS : CHECKER_ARGS)], {
      cwd: dir, stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...env, SOLID_CHECKER_DAEMON: "0", SOLID_CHECKER_RUNTIME_RESOLVER: env.SOLID_CHECKER_RUNTIME_RESOLVER ?? RESOLVER,
        ...(typefacts ?? env.SOLID_TYPEFACTS_BIN ? { SOLID_TYPEFACTS_BIN: typefacts ?? env.SOLID_TYPEFACTS_BIN } : {}),
      },
    });
    let stdout = ""; let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", (error) => { clearTimeout(timer); done({ ok: false, findings: [], wallMs: Date.now() - started, error: error.message }); });
    child.on("close", (status) => {
      clearTimeout(timer);
      const wallMs = Date.now() - started;
      // The oracle reads the native child's stdout as one JSON document; exit 0 and 1 both carry one.
      if (status !== 0 && status !== 1) { done({ ok: false, findings: [], wallMs, exitStatus: status, error: stderr.slice(-800) || `exit ${status}` }); return; }
      try {
        const analysis = JSON.parse(stdout);
        done({ ok: true, findings: parseFindings(analysis, dir), wallMs, exitStatus: status });
      } catch (error) {
        if (error instanceof UnmappedFindingsError) unmapped.push(error.message);
        done({ ok: false, findings: [], wallMs, exitStatus: status, error: error instanceof UnmappedFindingsError ? error.message : `malformed checker output: ${error.message}` });
      }
    });
  });
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
    .map(({ finding }) => finding)
    .map((finding) => ({
      file: "App.tsx", rule: finding.rule, id: finding.id, kind: finding.kind,
      line: finding.primaryLocation.line, endLine: finding.primaryLocation.endLine ?? finding.primaryLocation.line,
    }));
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
