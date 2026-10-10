// Evaluate unchanged retained applications through the reusable CLI.
// Setup failures and proof obligations are results, never detected defects.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const [outputArg, browserArg, phase = "all"] = process.argv.slice(2);
assert(outputArg && browserArg, "Usage: node evaluate-existing-applications.mjs <fresh-output-directory> <chromium-path>");
assert(["all", "native-only", "focused"].includes(phase), "Supported phases: all, native-only, focused (the two previously completed applications)");
const output = resolve(outputArg), browser = realpathSync(resolve(browserArg));
assert(!existsSync(output), "Use a fresh output directory");
assert(process.env.SOLID_CHECKER_NATIVE_BIN && process.env.SOLID_TYPEFACTS_BIN, "Set explicit fresh checker and Type Facts binaries");
mkdirSync(output, { recursive: true });
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const excluded = new Set(["node_modules", ".git", ".tanstack", ".vinxi", ".typefacts", "build", "dist", "coverage", "test-results", "playwright-report"]);
function applicationPins(root) {
  const pins = [];
  function visit(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (excluded.has(entry.name) || entry.name.startsWith(".env") || entry.name.endsWith(".tsbuildinfo")) continue;
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile()) pins.push({ path: relative(root, path), sha256: hash(readFileSync(path)) });
      else assert.fail(`Unsupported application input: ${path}`);
    }
  }
  visit(root);
  return pins.sort((a, b) => a.path.localeCompare(b.path));
}
function packageAt(root, name) {
  const require = createRequire(join(root, "package.json"));
  let path = realpathSync(require.resolve(name));
  for (let directory = dirname(path); ; directory = dirname(directory)) {
    const metadata = join(directory, "package.json");
    if (existsSync(metadata)) {
      const pkg = JSON.parse(readFileSync(metadata));
      if (pkg.name === name) return { root: directory, name, version: pkg.version, metadata, sha256: hash(readFileSync(metadata)) };
    }
    assert(dirname(directory) !== directory, `Cannot locate package ${name}`);
  }
}
const base = resolve("rust/target/app-import-metric/apps"), tooling = join(base, "helge-dev");
const selections = [
  { id: "helge-dev", root: join(base, "helge-dev"), config: "tsconfig.json", live: true },
  { id: "oscartbeaumont-website", root: join(base, "oscartbeaumont-website"), config: "tsconfig.app.json" },
  { id: "finds-team", root: join(base, "finds-team/frontend"), config: "tsconfig.json" },
];
const selectedApplications = phase === "focused" ? selections.filter(row => row.id !== "finds-team") : selections;
const implementationPaths = [fileURLToPath(import.meta.url), resolve("packages/cli/bin/solid-checker.mjs"), resolve("packages/cli/bin/launcher.mjs"),
  ...["development-feedback", "feedback-browser", "feedback-native-hook", "feedback-read-runtime", "feedback-read-selector", "feedback-source-hook", "feedback-assertion-selector"]
    .map(name => resolve(`packages/cli/scripts/${name}.mjs`)),
  realpathSync(process.env.SOLID_CHECKER_NATIVE_BIN), realpathSync(process.env.SOLID_TYPEFACTS_BIN), resolve("bin/solid-typefacts.buildinfo"), browser];
const implementation = implementationPaths.map(path => ({ path, sha256: hash(readFileSync(path)) }));
const report = { authority: false, certification: false, phase, startedAt: new Date().toISOString(), node: process.version,
  scope: `${selectedApplications.length} retained existing configurations with unchanged source/declarations. Executed checks are retained below. No seeded defects or repairs; this is not a representative accuracy study.`,
  implementation, results: [], existingTests: [] };
const save = () => writeFileSync(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n");
save();
function run(id, executable, args, { cwd = resolve("."), timeout = 120000 } = {}) {
  const started = performance.now();
  // macOS time reports process-tree peak RSS in bytes. Preserve its raw output;
  // this is a single-run observation, not a long-session memory guarantee.
  const measured = process.platform === "darwin";
  const child = spawnSync(measured ? "/usr/bin/time" : executable, measured ? ["-l", executable, ...args] : args,
    { cwd, encoding: "utf8", maxBuffer: 32 * 1024 * 1024, timeout, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" } });
  writeFileSync(join(output, id + ".stdout.log"), child.stdout ?? "");
  writeFileSync(join(output, id + ".stderr.log"), child.stderr ?? "");
  return { command: [executable, ...args], cwd, status: child.status, signal: child.signal,
    error: child.error && { code: child.error.code, message: child.error.message }, elapsedMs: performance.now() - started,
    maxResidentBytes: measured ? Number(child.stderr?.match(/(\d+)\s+maximum resident set size/)?.[1]) || null : null,
    stdout: child.stdout ?? "", stderr: child.stderr ?? "" };
}
function retainExecution(row) {
  const { stdout, stderr, ...execution } = row;
  return execution;
}
const scenario = { schemaVersion: 1, steps: [
  { action: "wait-for-text", selector: "main h1", text: "Helge Falch" },
  { action: "assert-text", id: "home-heading", selector: "main h1", text: "Helge Falch" },
  { action: "click", selector: '.innerContainer .buttons a[href="/projects"]' },
  { action: "wait-for-text", selector: ".projects h1", text: "Projects" },
  { action: "assert-text", id: "projects-heading", selector: ".projects h1", text: "Projects" },
  { action: "click", selector: '.innerContainer .buttons a[href="/about"]' },
  { action: "wait-for-text", selector: "main h1", text: "About" },
  { action: "assert-text", id: "about-heading", selector: "main h1", text: "About" },
  // Blog and Article load from dev.to. The collector blocks other origins, so
  // these steps run against supplied, digest-pinned responses (below).
  { action: "click", selector: '.innerContainer .buttons a[href="/blog"]' },
  { action: "wait-for-selector", selector: '.articles .article:has(a[href="/blog/101"]) h2' },
  { action: "assert-text", id: "blog-first-article", selector: '.articles .article:has(a[href="/blog/101"]) h2', text: "Supplied article one" },
  { action: "click", selector: '.articles .article a[href="/blog/101"]' },
  { action: "wait-for-text", selector: ".articleContainer .title a:not(.icon)", text: "Supplied article one" },
  { action: "assert-text", id: "article-title", selector: ".articleContainer .title a:not(.icon)", text: "Supplied article one" },
  { action: "click", selector: '.innerContainer .buttons a[href="/"]' },
  { action: "wait-for-text", selector: "main h1", text: "Helge Falch" },
  { action: "click", selector: '.icons [role="button"]' },
  { action: "wait-for-selector", selector: ".modal" },
  { action: "assert-text", id: "contact-address", selector: ".modal p", text: "helge.falch@gmail.com" },
], responses: [
  { url: "https://dev.to/api/articles?username=helgelol", body: "helge-dev.articles.json" },
  { url: "https://dev.to/api/articles/101", body: "helge-dev.article-101.json" }
] };
// Synthetic scenario inputs, not recorded dev.to data. The blacklisted id is
// included so the application's own filter has something to remove.
const suppliedBodies = {
  "helge-dev.articles.json": [
    { id: 101, title: "Supplied article one", description: "Scenario input", tags: "solid", category: "", link: "" },
    { id: 422939, title: "Blacklisted article", description: "Filtered by the application", tags: "", category: "", link: "" },
    { title: "External article", description: "", tags: "", category: "web", link: "https://example.test/post" }
  ],
  "helge-dev.article-101.json": { title: "Supplied article one", url: "https://example.test/post-101", body_html: "<p>Body</p>" }
};
for (const [name, body] of Object.entries(suppliedBodies)) writeFileSync(join(output, name), JSON.stringify(body, null, 2) + "\n");
const scenarioPath = join(output, "helge-dev.scenario.json");
writeFileSync(scenarioPath, JSON.stringify(scenario, null, 2) + "\n");
for (const selected of selectedApplications) {
  const root = realpathSync(selected.root), project = join(root, selected.config), inputs = applicationPins(root);
  const row = { id: selected.id, root, project, inputs, executions: {}, review: "pending" };
  report.results.push(row); save();
  const status = spawnSync("git", ["-C", root, "status", "--short"], { encoding: "utf8" });
  row.initialGitStatus = status.stdout;
  row.commit = spawnSync("git", ["-C", root, "rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();
  row.runtimePackages = ["solid-js", "@solidjs/web"].map(name => packageAt(root, name));
  const typescript = packageAt(root, "typescript"); row.typescript = typescript;
  const typing = phase !== "native-only" && run(selected.id + ".typing", process.execPath,
    [join(typescript.root, "bin/tsc"), "--noEmit", "--incremental", "false", "-p", project], { cwd: root });
  if (typing) row.executions.typing = retainExecution(typing);
  const feedback = run(selected.id + ".feedback", process.execPath,
    [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "--project", project]);
  row.executions.feedback = retainExecution(feedback);
  if ([0, 1].includes(feedback.status)) {
    const native = JSON.parse(feedback.stdout);
    writeFileSync(join(output, selected.id + ".feedback.json"), JSON.stringify(native, null, 2) + "\n");
    row.summary = { typingErrorCount: native.typingErrorCount, nativeViolations: native.findings.length,
      nativeGaps: native.gaps.length, observations: native.observations.length, runtimeCoverage: native.coverage.runtime };
  } else row.feedbackFailure = feedback.stderr;
  if (selected.live && phase !== "native-only") {
    const live = run(selected.id + ".live", process.execPath,
      [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "run", "--project", project,
        "--scenario", scenarioPath, "--browser", browser, "--tooling", tooling], { timeout: 90000 });
    row.executions.live = retainExecution(live);
    if ([0, 1].includes(live.status)) {
      const actual = JSON.parse(live.stdout);
      writeFileSync(join(output, selected.id + ".live.json"), JSON.stringify(actual, null, 2) + "\n");
      row.live = { assertionFailures: actual.assertionFailures, assertions: actual.assertions,
        automatic: actual.automatic, pageErrors: actual.execution.pageErrors, coverage: actual.coverage,
        observations: actual.observations, stats: actual.execution.stats };
    } else row.liveFailure = live.stderr;
  } else if (!selected.live) row.liveBlocker = { missingHtmlEntry: !existsSync(join(root, "index.html")),
    reason: "The client collector requires index.html; this application's server/generated-route entry needs an existing test capture or an SSR adapter." };
  assert.deepEqual(applicationPins(root), inputs, "Application inputs changed during evaluation");
  row.preservedInputs = true;
  row.finalGitStatus = spawnSync("git", ["-C", root, "status", "--short"], { encoding: "utf8" }).stdout;
  assert.equal(row.finalGitStatus, row.initialGitStatus, "Application worktree status changed");
  save(); console.log(JSON.stringify({ id: row.id, summary: row.summary, liveStatus: row.executions.live?.status, liveBlocker: row.liveBlocker }));
}
for (const selection of phase === "all" ? ["ui", "relay"] : []) {
  const directory = join(output, "finds-team-existing-" + selection);
  const execution = run("finds-team-existing-" + selection, process.execPath,
    [resolve("benchmarks/reviewed-package-models/real-app-existing-tests-v1.mjs"), directory, selection], { timeout: 180000 });
  report.existingTests.push({ selection, execution: retainExecution(execution),
    results: existsSync(join(directory, "results.json")) ? JSON.parse(readFileSync(join(directory, "results.json"))) : null });
  save(); console.log(JSON.stringify({ selection, status: execution.status }));
}
for (const pin of implementation) assert.equal(hash(readFileSync(pin.path)), pin.sha256, `Implementation input changed: ${pin.path}`);
for (const row of report.results) {
  assert.deepEqual(applicationPins(row.root), row.inputs, "Application inputs changed");
  for (const pkg of [...row.runtimePackages, row.typescript]) assert.equal(hash(readFileSync(pkg.metadata)), pkg.sha256);
}
report.finishedAt = new Date().toISOString(); report.preservedInputs = true; save();
