// The checker adapter: materializes a case the way scripts/runtime-oracle.mjs does
// (App.tsx, the oracle's main.tsx, the oracle's tsconfig, inside the pinned install so
// its node_modules is an ancestor), runs the native checker as `feedback run` does
// (`--format json --feedback-facts` for the development runtime), and reduces its
// findings to a verdict per site: violation, uncertifiable, or none.
//
// Uncertifiable is an internal result (ADR 0328); here it means "the checker said
// nothing it can prove", which is never evidence either way.
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { ignoredRules } from "./codes.mjs";
import { tsconfigFor } from "./typecheck.mjs";

/** The oracle's entry for a case with no behavioural observation (scripts/runtime-oracle.mjs `mainSource([])`). */
export const MAIN_SOURCE = `import { render } from "@solidjs/web";
import * as mod from "./App";

// Each exported component is rendered as a component, so Solid creates it the
// way an application does, with the component's strict-read window open, and
// in its own root, so one component's failure does not hide the others.
const root = document.getElementById("root")!;
for (const [name, value] of Object.entries(mod as Record<string, unknown>)) {
  if (typeof value !== "function" || !(name === "default" || /^[A-Z]/.test(name))) continue;
  const Component = value as (props: {}) => any;
  const host = document.createElement("section");
  host.dataset.export = name;
  root.append(host);
  try {
    render(() => <Component />, host);
  } catch (error) {
    console.error("render failed", name, error);
    // Rethrown from a timer, so the uncaught error reaches the feedback run's
    // source mapping as a runtime exception (ADR 0352); the others still render.
    setTimeout(() => { throw error; });
  }
}
`;

/** Where checker cases live: inside the install, which the oracle's own cases also use. */
export const checkerWorkDir = (install, id) => join(resolve(install), ".solid-checker-fuzz", id);

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
  return dir;
}

/** Copies a case directory (App.tsx etc.) into the install's checker work area. */
export function materializeFromDirectory(caseDir, install, id) {
  const dir = checkerWorkDir(install, id);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  cpSync(join(caseDir, "App.tsx"), join(dir, "App.tsx"));
  writeFileSync(join(dir, "main.tsx"), MAIN_SOURCE);
  writeFileSync(join(dir, "tsconfig.json"), tsconfigFor(["main.tsx", "App.tsx"]));
  return dir;
}

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
/** The runtime resolver the launcher (packages/cli/bin/launcher.mjs) points the native checker at. */
const RESOLVER = join(repo, "packages/cli/scripts/runtime-resolver.mjs");

const CHECKER_ARGS = ["--format", "json", "--feedback-facts", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];

/**
 * Runs the native checker on a materialized directory.
 * `bin` and `typefacts` set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN (the same variables the oracle needs).
 * @returns {Promise<{ ok: boolean, findings: object[], wallMs: number, error?: string, exitStatus?: number }>}
 */
export function runChecker(dir, { bin, typefacts, timeoutMs = 180000, env = process.env } = {}) {
  return new Promise((done) => {
    const executable = bin ?? env.SOLID_CHECKER_NATIVE_BIN;
    if (!executable || !existsSync(executable)) { done({ ok: false, findings: [], wallMs: 0, error: `checker binary not found: ${executable}` }); return; }
    const started = Date.now();
    const child = spawn(executable, ["--project", join(dir, "tsconfig.json"), ...CHECKER_ARGS], {
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
        done({ ok: false, findings: [], wallMs, exitStatus: status, error: `malformed checker output: ${error.message}` });
      }
    });
  });
}

/** The findings in App.tsx, as the oracle reads them (path, rule, id, kind, line range). */
export function parseFindings(analysis, dir) {
  if (!analysis || !Array.isArray(analysis.findings)) throw new Error("no findings array");
  const prefix = `${resolve(dir)}/`;
  return analysis.findings
    .filter((finding) => finding.primaryLocation?.path?.startsWith(prefix) && relative(dir, finding.primaryLocation.path) === "App.tsx")
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
