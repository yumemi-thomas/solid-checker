import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { captureTemplate, feedbackSnapshot, inspectDevelopmentFeedback, validateFeedbackInputs } from "./development-feedback.mjs";
import { instrumentFeedbackReads, sharedReaderSha256 } from "./feedback-native-hook.mjs";
import { selectAssertionFeedback } from "./feedback-assertion-selector.mjs";
import { instrumentFeedbackSource } from "./feedback-source-hook.mjs";
import { selectReadFeedback } from "./feedback-read-selector.mjs";
import { classifyUnmappedFrames, createFrameAttributor, summarizeCandidateScopes, summarizeUnmapped } from "./feedback-attribution.mjs";

const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const runtimePath = fileURLToPath(new URL("./feedback-read-runtime.mjs", import.meta.url));

export function validateFeedbackScenario(scenario) {
  assert.equal(scenario.schemaVersion, 1);
  assert(Array.isArray(scenario.steps) && scenario.steps.length > 0 && scenario.steps.length <= 64, "Scenario needs 1–64 steps");
  const ids = new Set();
  for (const step of scenario.steps) {
    assert(["click", "wait-for-selector", "wait-for-text", "assert-text"].includes(step.action), "Unsupported scenario action");
    assert(typeof step.selector === "string" && step.selector.trim(), "Step needs a selector");
    if (["wait-for-text", "assert-text"].includes(step.action)) assert.equal(typeof step.text, "string");
    if (step.action === "assert-text") {
      assert(typeof step.id === "string" && step.id && !ids.has(step.id), "Assertion needs a unique ID"); ids.add(step.id);
    }
  }
  return scenario;
}

async function toolingAt(root) {
  const require = createRequire(join(root, "package.json"));
  const imported = entry => {
    if (typeof entry === "string") return entry;
    if (Array.isArray(entry)) return entry.map(imported).find(Boolean);
    if (entry && typeof entry === "object") return imported(entry.import ?? entry.node ?? entry.default);
    return null;
  };
  const load = async name => {
    const resolved = require.resolve(name);
    let directory = dirname(resolved);
    while (dirname(directory) !== directory) {
      const metadata = join(directory, "package.json");
      if (existsSync(metadata)) {
        const pkg = JSON.parse(readFileSync(metadata, "utf8"));
        if (pkg.name === name) {
          const entry = imported(pkg.exports?.["."]);
          return import(pathToFileURL(entry ? resolve(directory, entry) : resolved));
        }
      }
      directory = dirname(directory);
    }
    return import(pathToFileURL(resolved));
  };
  // The Solid plugin's CJS fallback requires Vite. Finish that ESM load before
  // loading dependent tooling, including older packages with mixed exports.
  const { createServer, loadConfigFromFile, mergeConfig } = await load("vite");
  const [{ chromium }, mapping] = await Promise.all([load("playwright"), load("@jridgewell/trace-mapping")]);
  return { createServer, loadConfigFromFile, mergeConfig,
    solid: async options => (await load("@solidjs/vite-plugin")).default(options), chromium,
    TraceMap: mapping.TraceMap, originalPositionFor: mapping.originalPositionFor };
}

export async function loadFeedbackConfiguration(tools, projectRoot, record) {
  const loaded = await tools.loadConfigFromFile({ command: "serve", mode: "development" }, undefined, projectRoot, "silent");
  if (!loaded) return { config: { publicDir: false }, plugins: [await tools.solid({ hot: false })],
    root: projectRoot, configuration: { kind: "collector-default", path: null } };
  for (const path of new Set([loaded.path, ...loaded.dependencies])) record(path);
  return { config: loaded.config, plugins: [], root: resolve(projectRoot, loaded.config.root ?? "."),
    configuration: { kind: "application-vite-config", path: realpathSync(loaded.path) } };
}

export function mergeFeedbackServerConfiguration(tools, application, collector) {
  const merged = tools.mergeConfig(application, collector);
  // Capture hooks must see original source before an application compiler's
  // `pre` transform. Vite's ordinary merge concatenates application plugins
  // first, which would make instrumentation inspect already compiled code.
  merged.plugins = [...(collector.plugins ?? []), ...(application.plugins ?? [])];
  return merged;
}

// A failed readiness step is part of the executed result. Preserve earlier
// assertions and collect the browser's errors instead of discarding them when
// a locator times out. No later interaction runs after a failed prerequisite.
export async function executeFeedbackScenario(page, scenario) {
  const assertions = [], checkpoints = [];
  for (const [index, step] of scenario.steps.entries()) {
    try {
      if (step.action === "click") await page.locator(step.selector).click();
      if (step.action === "wait-for-selector") await page.locator(step.selector).waitFor({ state: "visible" });
      if (step.action === "wait-for-text") await page.waitForFunction(({ selector, text }) =>
        document.querySelector(selector)?.textContent === text, { selector: step.selector, text: step.text });
      if (step.action === "assert-text") {
        const actual = await page.locator(step.selector).textContent();
        assertions.push({ id: step.id, selector: step.selector, expected: step.text, actual,
          passed: actual === step.text, channel: "executed-assertion", severity: actual === step.text ? "info" : "error",
          authority: false, certification: false });
      }
      checkpoints.push(await page.evaluate(selector => ({ events: globalThis.__solidCheckerReads?.events.length ?? 0,
        selector, text: document.querySelector(selector)?.textContent ?? null }), step.selector));
    } catch (error) {
      return { assertions, checkpoints, failure: { step: index, action: step.action, selector: step.selector,
        assertionId: step.id ?? null, message: error.message, severity: "error", authority: false, certification: false } };
    }
  }
  return { assertions, checkpoints, failure: null };
}

// Vite transform boundary: only one byte-reviewed native artifact is edited.
// Every other runtime module remains original code executed by the application.
export function feedbackReadPlugin({ snapshot, runtimeInputs, coverage }) {
  const virtual = "\0solid-checker-feedback-entry", publicId = "virtual:solid-checker-feedback-entry";
  return {
    name: "solid-checker-feedback-reads", enforce: "pre",
    resolveId(id) { if (id === publicId) return virtual; },
    load(id) {
      if (id !== virtual) return;
      return `import {reads} from ${JSON.stringify("/@fs" + runtimePath)};
        import * as Solid from 'solid-js';
        globalThis.__solidCheckerDiagnostics=[];
        Solid.OBSERVE?.diagnostics?.subscribe(event=>{
          const rows=globalThis.__solidCheckerDiagnostics;
          if(rows.length<256)rows.push({code:event.code,kind:event.kind,severity:event.severity});
        });`;
    },
    transformIndexHtml() { return [{ tag: "script", attrs: { type: "module", src: "/@id/" + publicId }, injectTo: "head-prepend" }]; },
    transform(code, id) {
      const path = id.split("?")[0];
      if (!path.endsWith("/dist/dev-shared.js")) return null;
      // Resolve package identity from this exact module, including pnpm paths.
      const metadata = join(dirname(dirname(path)), "package.json");
      if (!existsSync(metadata)) return null;
      const pkg = JSON.parse(readFileSync(metadata, "utf8"));
      if (pkg.name !== "@solidjs/signals") return null;
      assert.equal(pkg.version, "2.0.0-rc.9", "Native read collector needs a reviewed runtime profile");
      assert.equal(readFileSync(path, "utf8"), code, "A previous transform changed the native reader");
      const result = instrumentFeedbackReads(code, path, "/@fs" + runtimePath);
      runtimeInputs.set(realpathSync(metadata), { path: realpathSync(metadata), sha256: hash(readFileSync(metadata)) });
      coverage.nativeReader = { path: realpathSync(path), sha256: sharedReaderSha256, version: pkg.version };
      return result;
    },
    configResolved(config) {
      // The framework plugin adds prebundled includes during config merging.
      // Bypass them so reviewed reader bytes execute through this hook and
      // ephemeral optimizer output cannot masquerade as a package artifact.
      for (const options of [config.optimizeDeps, ...Object.values(config.environments ?? {}).map(row => row.optimizeDeps)].filter(Boolean)) {
        options.include = []; options.noDiscovery = true;
        options.exclude = [...new Set([...(options.exclude ?? []), "solid-js", "@solidjs/signals", "@solidjs/web"])];
      }
      validateFeedbackInputs(snapshot.manifest);
    }
  };
}

async function collectProject(project, scenario, tools, browser) {
  const snapshot = feedbackSnapshot(project), projectRoot = dirname(snapshot.manifest.project);
  const nativeFeedback = inspectDevelopmentFeedback(project, { feedbackFacts: true, snapshot });
  assert(Array.isArray(nativeFeedback.analysis.feedbackFacts), "Native checker does not expose development models");
  const models = new Map(nativeFeedback.analysis.feedbackFacts.map(model => [model.path, model]));
  const runtimeInputs = new Map(), coverage = { nativeReader: null, sourceInstrumentation: [], complete: false }, unmapped = [];
  const instrumented = new Map();
  const sourceFiles = new Map(snapshot.sources.filter(source => !source.isDeclarationFile)
    .map(source => [resolve(source.fileName), source]));
  const record = path => {
    path = realpathSync(path);
    const pin = { path, sha256: hash(readFileSync(path)) }, previous = runtimeInputs.get(path);
    assert(!previous || previous.sha256 === pin.sha256, `Runtime input changed: ${path}`);
    runtimeInputs.set(path, pin);
  };
  const configuration = await loadFeedbackConfiguration(tools, projectRoot, record), root = configuration.root;
  assert(existsSync(join(root, "index.html")), "Browser feedback needs a client index.html; use an existing application test capture for server-rendered applications");
  for (const path of [runtimePath, join(root, "index.html")]) record(path);
  const cache = mkdtempSync(join(tmpdir(), "solid-feedback-vite-"));
  const server = await tools.createServer(mergeFeedbackServerConfiguration(tools, configuration.config, { configFile: false, root, cacheDir: cache,
    plugins: [feedbackReadPlugin({ snapshot, runtimeInputs, coverage }), {
      name: "solid-checker-development-source", enforce: "pre",
      transform(code, id) {
        const path = id.split("?")[0], model = models.get(path);
        if (!model || snapshot.typingErrors || !model.functions.length) return null;
        const result = instrumentFeedbackSource(code, path, model, "/@fs" + runtimePath);
        coverage.sourceInstrumentation.push({ path, ...result.counts });
        instrumented.set(path, result.instrumented);
        return { code: result.code, map: result.map };
      }
    }, ...configuration.plugins, {
      name: "solid-checker-feedback-inputs", enforce: "post",
      transform(_code, id) {
        const path = id.split("?")[0];
        if (existsSync(path) && /\.[cm]?[jt]sx?$/.test(path)) record(path);
      }
    }],
    resolve: { dedupe: ["solid-js", "@solidjs/signals", "@solidjs/web"] },
    optimizeDeps: { noDiscovery: true, include: [], exclude: ["solid-js", "@solidjs/signals", "@solidjs/web"] },
    server: { host: "127.0.0.1", port: 0, fs: { allow: [root, dirname(runtimePath), dirname(dirname(runtimePath)), tools.root] } },
    logLevel: "silent" }));
  let context;
  const pageErrors = [], blockedRequests = [], consoleDiagnostics = [];
  let consoleDiagnosticsDropped = 0;
  try {
    await server.listen();
    const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
    context = await browser.newContext(); context.setDefaultTimeout(12000);
    await context.route("**/*", route => {
      const url = new URL(route.request().url());
      if (url.origin === origin) return route.continue();
      blockedRequests.push(url.href); return route.abort();
    });
    const page = await context.newPage();
    page.on("pageerror", error => pageErrors.push({ message: error.message, stack: error.stack }));
    page.on("console", message => {
      if (!["error", "warning"].includes(message.type())) return;
      if (consoleDiagnostics.length >= 256) { consoleDiagnosticsDropped++; return; }
      consoleDiagnostics.push({ severity: message.type(), message: message.text(), location: message.location() });
    });
    await page.goto(new URL(server.config.base, origin).href, { waitUntil: "domcontentloaded" });
    const { assertions, checkpoints, failure } = await executeFeedbackScenario(page, scenario);
    const raw = await page.evaluate(() => ({ events: globalThis.__solidCheckerReads?.events ?? [],
      stats: globalThis.__solidCheckerReads?.stats ?? null, scopes: globalThis.__solidCheckerReads?.scopes ?? { rows: [], dropped: 0 },
      diagnostics: globalThis.__solidCheckerDiagnostics ?? [] }));
    const capture = captureTemplate(snapshot), maps = new Map();
    const attribute = createFrameAttributor({ origin, root, sourceFiles, collectorPaths: [realpathSync(runtimePath)],
      originalPositionFor: tools.originalPositionFor,
      async mapFor(key) {
        if (!maps.has(key)) {
          const transformed = await server.transformRequest(key);
          maps.set(key, transformed?.map ? new tools.TraceMap(transformed.map) : null);
        }
        return maps.get(key);
      } });
    function site({ source, original: { path, line, column } }) {
      const lines = source.text.split("\n");
      const offset = lines.slice(0, line - 1).reduce((n, row) => n + row.length + 1, 0) + column - 1;
      const character = String.fromCodePoint(source.text.codePointAt(offset));
      return { location: { path, startByte: Buffer.byteLength(source.text.slice(0, offset)),
        endByte: Buffer.byteLength(source.text.slice(0, offset)) + Buffer.byteLength(character) },
        sourceSha256: hash(source.text), line, column };
    }
    // The first mapped frame is the authored site. Otherwise every frame's
    // outcome is retained and the record is classified without a site.
    async function locate(frames, stackTruncated) {
      const attributions = [];
      for (const frame of frames) {
        const row = await attribute(frame);
        if (row.outcome === "mapped") return { site: site(row) };
        attributions.push(row);
      }
      return { unmapped: classifyUnmappedFrames(frames, attributions, { stackTruncated }) };
    }
    const authoredEvents = [];
    for (const [index, event] of raw.events.entries()) {
      const located = coverage.nativeReader ? await locate(event.frames, event.stackTruncated ?? null) :
        { unmapped: { attribution: "no-reviewed-reader", reason: "The reviewed native reader was not instrumented", frames: event.frames } };
      if (!located.site) { unmapped.push({ kind: event.kind ?? "untracked-read", nodeId: event.nodeId, ...located.unmapped }); continue; }
      const site = located.site;
      authoredEvents.push({ ...event, site });
      capture.events.push({ id: `read-${index}`, kind: event.kind ?? "untracked-read", message: event.kind === "observer-query" ?
        "Queried tracking and found no observer. Whether package code skipped subscribing remains open." :
        "Executed a reactive read without an observer or owner. Intent remains open.",
        tracking: "untracked", runtimeInput: coverage.nativeReader.path, ...site });
    }
    // Uncaught errors remain recorded failures. Source attribution is accepted
    // only when the browser stack maps to a configured original source.
    // A page error's stack depth is the page's own limit, so its completeness
    // is unknown and it can never be classified as package frames only.
    for (const [index, error] of pageErrors.entries()) {
      const frames = (error.stack ?? "").split("\n").flatMap(line => {
        const match = line.match(/(https?:\/\/.*?):(\d+):(\d+)\)?$/);
        return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
      });
      const located = await locate(frames, null);
      if (located.site) capture.events.push({ id: `error-${index}`, kind: "runtime-exception", message: error.message, ...located.site });
      else unmapped.push({ kind: "runtime-exception", message: error.message, ...located.unmapped });
    }
    coverage.candidateScopes = summarizeCandidateScopes([...models.values()], { instrumented, scopes: raw.scopes,
      lineOf(path, byte) {
        const text = sourceFiles.get(path)?.text;
        return text === undefined ? null : Buffer.from(text).subarray(0, byte).toString().split("\n").length;
      } });
    coverage.unmapped = summarizeUnmapped(unmapped);
    capture.runtimeInputs = [...runtimeInputs.values()];
    for (const pin of capture.runtimeInputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256, `Runtime input changed: ${pin.path}`);
    validateFeedbackInputs(snapshot.manifest);
    const feedback = inspectDevelopmentFeedback(project, { capture, snapshot, analyze: () => ({ status: nativeFeedback.nativeExitCode,
      stdout: JSON.stringify(nativeFeedback.analysis) }) });
    const automatic = selectReadFeedback(authoredEvents, [...models.values()], { typingErrors: snapshot.typingErrors, dropped: raw.stats?.dropped ?? 0 });
    return { feedback, capture, assertions: snapshot.typingErrors ? [] : assertions,
      automatic,
      coverage, stats: raw.stats, diagnostics: raw.diagnostics, checkpoints, unmapped, blockedRequests,
      pageErrors, consoleDiagnostics, consoleDiagnosticsDropped, failure,
      configuration: configuration.configuration, typingErrors: snapshot.typingErrors };
  } finally { await context?.close(); await server.close(); rmSync(cache, { recursive: true, force: true }); }
}

export async function runBrowserFeedback({ project, scenarioPath, browserPath, toolingRoot, comparisonProject }) {
  const scenarioPin = { path: realpathSync(resolve(scenarioPath)), sha256: hash(readFileSync(scenarioPath)) };
  const scenario = validateFeedbackScenario(JSON.parse(readFileSync(scenarioPin.path, "utf8")));
  const tools = await toolingAt(resolve(toolingRoot ?? dirname(resolve(project))));
  tools.root = realpathSync(resolve(toolingRoot ?? dirname(resolve(project))));
  const browserPin = { path: realpathSync(resolve(browserPath)), sha256: hash(readFileSync(browserPath)) };
  const browser = await tools.chromium.launch({ executablePath: browserPin.path, headless: true, timeout: 15000 });
  try {
    const original = await collectProject(project, scenario, tools, browser);
    const comparison = comparisonProject ? await collectProject(comparisonProject, scenario, tools, browser) : null;
    for (const run of [original, comparison].filter(Boolean)) {
      validateFeedbackInputs(run.capture.manifest);
      for (const pin of run.capture.runtimeInputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256, `Runtime input changed: ${pin.path}`);
    }
    assert.equal(hash(readFileSync(scenarioPin.path)), scenarioPin.sha256, "Scenario changed during execution");
    assert.equal(hash(readFileSync(browserPin.path)), browserPin.sha256, "Browser changed during execution");
    const selection = selectAssertionFeedback(original.assertions, comparison?.assertions ?? []);
    return { ...original.feedback, coverage: { ...original.feedback.coverage, runtime: "executed-browser", nativeReadCollection: original.coverage },
      assertions: original.assertions, assertionFailures: original.assertions.filter(row => !row.passed),
      guidance: selection, execution: { ...original, feedback: undefined, capture: undefined },
      automatic: original.automatic,
      comparison: comparison ? { project: comparison.feedback.project, inputId: comparison.feedback.inputId,
        assertions: comparison.assertions, stats: comparison.stats, nativeExitCode: comparison.feedback.nativeExitCode,
        typingErrors: comparison.typingErrors, pageErrors: comparison.pageErrors } : null,
      scenario: scenarioPin, browser: { ...browserPin, version: browser.version() } };
  } finally { await browser.close(); }
}
