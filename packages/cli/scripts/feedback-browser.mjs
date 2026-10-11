import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { createRequire, isBuiltin } from "node:module";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { captureTemplate, feedbackBrowserSnapshots, inspectDevelopmentFeedback, validateFeedbackInputs } from "./development-feedback.mjs";
import { instrumentFeedbackReads, readerProfiles } from "./feedback-native-hook.mjs";
import { selectAssertionFeedback } from "./feedback-assertion-selector.mjs";
import { instrumentFeedbackSource } from "./feedback-source-hook.mjs";
import { selectReadFeedback } from "./feedback-read-selector.mjs";
import { classifyUnmappedFrames, createFrameAttributor, operationFrame, siteExpressionKind, summarizeCandidateScopes, summarizeUnmapped } from "./feedback-attribution.mjs";
import { admitFeedbackEntry, observeFeedbackEntry } from "./feedback-entry.mjs";
import ts from "typescript";

const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const runtimePath = fileURLToPath(new URL("./feedback-read-runtime.mjs", import.meta.url));

export function validateFeedbackScenario(scenario) {
  assert.equal(scenario.schemaVersion, 1);
  assert(Array.isArray(scenario.steps) && scenario.steps.length > 0 && scenario.steps.length <= 64, "Scenario needs 1–64 steps");
  const ids = new Set();
  for (const step of scenario.steps) {
    assert(["click", "wait-for-selector", "wait-for-text", "assert-text", "fill", "type", "press", "select", "check", "uncheck", "submit"].includes(step.action), "Unsupported scenario action");
    assert(typeof step.selector === "string" && step.selector.trim(), "Step needs a selector");
    assert(!step.selector.includes("=") || !/^(?:\w+)=/.test(step.selector), "Selectors must be CSS, not Playwright locator engines");
    if (["fill", "type"].includes(step.action)) assert.equal(typeof step.text, "string", "Input action needs text");
    if (step.action === "press") assert(typeof step.key === "string" && step.key.trim(), "Press needs a key");
    if (step.action === "select") assert.equal(typeof step.value, "string", "Select needs an option value");
    if (["wait-for-text", "assert-text"].includes(step.action)) assert.equal(typeof step.text, "string");
    if (step.action === "assert-text") {
      assert(typeof step.id === "string" && step.id && !ids.has(step.id), "Assertion needs a unique ID"); ids.add(step.id);
    }
  }
  if (scenario.viewport !== undefined) {
    const viewport = scenario.viewport;
    assert(viewport && typeof viewport === "object" && !Array.isArray(viewport), "Viewport needs an object");
    assert(Object.keys(viewport).every(key => ["width", "height", "deviceScaleFactor", "isMobile"].includes(key)), "Unsupported viewport field");
    for (const key of ["width", "height"]) assert(Number.isInteger(viewport[key]) && viewport[key] > 0 && viewport[key] <= 16384, "Viewport dimensions must be integers in 1–16384");
    assert(viewport.deviceScaleFactor === undefined || (Number.isFinite(viewport.deviceScaleFactor) && viewport.deviceScaleFactor > 0 && viewport.deviceScaleFactor <= 8), "Unsupported device scale factor");
    assert(viewport.isMobile === undefined || typeof viewport.isMobile === "boolean", "isMobile must be boolean");
  }
  const responses = scenario.responses ?? [], urls = new Set();
  assert(Array.isArray(responses) && responses.length <= 64, "Scenario responses need an array of at most 64 entries");
  for (const response of responses) {
    assert(typeof response.url === "string", "A supplied response needs a URL or same-origin path + query");
    const sameOrigin = response.url.startsWith("/") && !response.url.startsWith("//");
    let url; try { url = new URL(response.url, sameOrigin ? "http://scenario.invalid" : undefined); } catch { assert.fail("A supplied response needs an absolute URL or same-origin path + query"); }
    assert(["http:", "https:"].includes(url.protocol) && !response.url.includes("#") && !url.username && !url.password
      && (sameOrigin ? url.origin === "http://scenario.invalid" && url.pathname + url.search === response.url : url.href === response.url), "A supplied response needs an exact, normalized http(s) URL or path + query");
    const method = response.method ?? "GET";
    assert(["GET", "POST"].includes(method), "Supplied responses support GET or POST only");
    assert(method === "POST" ? typeof response.requestBody === "string" : response.requestBody === undefined,
      "POST needs an explicit requestBody match; GET must not have one");
    const key = responseKey(response.url, method, response.requestBody);
    assert(!urls.has(key), "Supplied response URLs/methods/body matches must be unique"); urls.add(key);
    assert(typeof response.body === "string" && response.body, "A supplied response needs a body file");
    assert(response.status === undefined || (Number.isInteger(response.status) && response.status >= 200 && response.status <= 599), "Unsupported response status");
    assert(response.contentType === undefined || typeof response.contentType === "string", "contentType must be a string");
  }
  return scenario;
}

const responseKey = (url, method = "GET", body) => method === "GET" ? url : JSON.stringify([url, method, body]);

export function feedbackContextOptions(scenario) {
  if (!scenario.viewport) return {};
  const { width, height, ...device } = scenario.viewport;
  return { viewport: { width, height }, ...device };
}

// Supplied responses are scenario inputs, never observed network behaviour.
// Their exact targets and bodies are pinned; other cross-origin traffic stays
// blocked while unmatched same-origin traffic keeps application behavior.
export function loadSuppliedResponses(scenario, scenarioPath) {
  return new Map((scenario.responses ?? []).map(response => {
    const path = realpathSync(resolve(dirname(scenarioPath), response.body)), body = readFileSync(path);
    return [responseKey(response.url, response.method, response.requestBody), { url: response.url, method: response.method ?? "GET",
      ...(response.requestBody !== undefined ? { requestBody: response.requestBody, requestBodySha256: hash(response.requestBody) } : {}),
      status: response.status ?? 200, contentType: response.contentType ?? "application/json",
      body, pin: { path, sha256: hash(body) }, served: 0 }];
  }));
}

// Exact same-origin matches are fulfilled before the browser can reach Vite's
// proxy. Unmatched application traffic keeps the existing server behavior.
export function routeFeedbackRequest(route, origin, responses, blockedRequests) {
  const request = route.request(), url = new URL(request.url()), sameOrigin = url.origin === origin;
  const target = sameOrigin ? url.pathname + url.search : url.href;
  const supplied = responses.get(responseKey(target, request.method(), request.postData() ?? ""));
  if (supplied) {
    supplied.served++;
    return route.fulfill({ status: supplied.status, contentType: supplied.contentType, body: supplied.body,
      headers: { "access-control-allow-origin": origin } });
  }
  if (sameOrigin) return route.continue();
  blockedRequests.push(url.href); return route.abort();
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

export async function loadFeedbackConfiguration(tools, projectRoot, record, options = {}, logger) {
  const loaded = await tools.loadConfigFromFile({ command: "serve", mode: "development" }, options.config, options.root ?? projectRoot, "silent", logger);
  assert(!options.config || loaded, "Explicit Vite config could not be loaded");
  if (!loaded) return { config: { publicDir: false }, plugins: [await tools.solid({ hot: false })],
    root: options.root ?? projectRoot, configuration: { kind: "collector-default", path: null, root: options.root ?? projectRoot } };
  const dependencies = [...new Set([loaded.path, ...loaded.dependencies])];
  for (const path of dependencies) record(path);
  const root = options.root ?? resolve(projectRoot, loaded.config.root ?? ".");
  return { config: loaded.config, plugins: [], root, dependencies,
    configuration: { kind: "application-vite-config", path: realpathSync(loaded.path), root } };
}

// Vite's config bundle externalizes package imports, so its dependencies list
// alone does not pin a middleware generator's installed implementation. Pin
// the exact resolved artifact, package metadata and root-export artifacts
// (including import/require branches), plus their local helper modules.
export function recordFeedbackGeneratorInputs(configFiles, record) {
  const visited = new Set(), recorded = new Set();
  const targets = value => typeof value === "string" ? [value] : value && typeof value === "object" ? Object.values(value).flatMap(targets) : [];
  const visit = (path, configuration = false) => {
    path = realpathSync(path);
    const key = JSON.stringify([path, configuration]);
    if (visited.has(key)) return;
    visited.add(key);
    if (!recorded.has(path)) { recorded.add(path); record(path); }
    if (!/\.[cm]?[jt]sx?$/.test(path)) return;
    const require = createRequire(path);
    for (const { fileName: specifier } of ts.preProcessFile(readFileSync(path, "utf8"), true, true).importedFiles) {
      if (isBuiltin(specifier) || (!configuration && !specifier.startsWith("."))) continue;
      let resolved;
      try { resolved = require.resolve(specifier); } catch { continue; } // Vite owns unresolved/conditional config imports.
      visit(resolved);
      if (specifier.startsWith(".") || isAbsolute(specifier)) continue;
      let directory = dirname(resolved);
      while (dirname(directory) !== directory) {
        const metadata = join(directory, "package.json");
        if (existsSync(metadata)) {
          const pkg = JSON.parse(readFileSync(metadata, "utf8"));
          record(metadata);
          if (pkg.name === specifier) for (const target of targets(pkg.exports?.["."] ?? pkg.exports)) {
            if (!target.startsWith("./") || target.includes("*")) continue;
            const artifact = resolve(directory, target);
            if (existsSync(artifact) && statSync(artifact).isFile()) visit(artifact);
          }
          if (typeof pkg.name === "string") break;
        }
        directory = dirname(directory);
      }
    }
  };
  for (const path of configFiles) visit(path, true);
}

export function createFeedbackDiagnostics() {
  const vite = [], failedModules = [], pending = new Set();
  let viteDropped = 0, failedModulesDropped = 0;
  const bounded = text => { const bytes = Buffer.from(String(text)); return { text: bytes.subarray(0, 16384).toString(), bytes: bytes.length, truncated: bytes.length > 16384 }; };
  const log = (severity, message) => {
    if (vite.length >= 64) { viteDropped++; return; }
    vite.push({ severity, ...bounded(message) });
  };
  return {
    logger: { hasWarned: false, info() {}, warn(message) { this.hasWarned = true; log("warning", message); },
      warnOnce(message) { this.warn(message); }, error(message) { log("error", message); }, clearScreen() {}, hasErrorLogged() { return false; } },
    response(response, origin) {
      if (new URL(response.url()).origin !== origin || response.status() < 400 || response.request().resourceType() !== "script") return;
      if (failedModules.length >= 64) { failedModulesDropped++; return; }
      const row = { url: response.url(), status: response.status(), contentType: response.headers()["content-type"] ?? null };
      failedModules.push(row);
      const work = (async () => {
        try { row.body = bounded((await response.body()).toString()); }
        catch (error) { row.bodyUnavailable = bounded(error.message).text; }
      })();
      pending.add(work); work.finally(() => pending.delete(work));
    },
    async finish() {
      await Promise.all([...pending]);
      return { vite, viteDropped, failedModules, failedModulesDropped };
    }
  };
}

// Every resource is attempted even if an earlier close rejects. This also
// covers createServer/listen failures and removes only this run's cache tree.
export async function closeFeedbackResources({ context, server, cache, browser }) {
  const errors = [];
  for (const resource of [context, server, browser]) {
    try { await resource?.close(); } catch (error) { errors.push(error); }
  }
  if (cache) rmSync(cache, { recursive: true, force: true });
  if (errors.length) throw new AggregateError(errors, "Feedback resource cleanup failed");
}

const inside = (tree, path) => { const part = relative(tree, path); return part !== ".." && !part.startsWith(".." + sep) && !isAbsolute(part); };

// A repository/workspace boundary admits configs above a package's source
// leaf; without one, the nearest package directory establishes the app tree.
export function validateFeedbackRuntimeOptions(project, { root, config } = {}) {
  let appTree = dirname(realpathSync(resolve(project))), cursor = appTree, foundPackage = false;
  const boundaryInputs = [];
  while (dirname(cursor) !== cursor) {
    const metadata = join(cursor, "package.json");
    if (existsSync(metadata)) {
      if (!foundPackage) { appTree = cursor; foundPackage = true; boundaryInputs.push(metadata); }
      const pkg = JSON.parse(readFileSync(metadata, "utf8"));
      if (pkg.workspaces || existsSync(join(cursor, "pnpm-workspace.yaml"))) {
        appTree = cursor; boundaryInputs.push(metadata);
        if (existsSync(join(cursor, "pnpm-workspace.yaml"))) boundaryInputs.push(join(cursor, "pnpm-workspace.yaml"));
        break;
      }
    }
    if (existsSync(join(cursor, ".git"))) { appTree = cursor; break; }
    cursor = dirname(cursor);
  }
  const validate = (path, directory) => {
    if (path === undefined) return undefined;
    const canonical = realpathSync(resolve(path));
    assert(inside(appTree, canonical), `Explicit Vite input is outside the app tree: ${canonical}`);
    assert(directory ? statSync(canonical).isDirectory() : statSync(canonical).isFile(), "Vite root must be a directory and config must be a file");
    return canonical;
  };
  return { appTree, boundaryInputs: [...new Set(boundaryInputs)], root: validate(root, true), config: validate(config, false) };
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
      if (step.action === "fill") await page.locator(step.selector).fill(step.text);
      if (step.action === "type") await page.locator(step.selector).pressSequentially(step.text);
      if (step.action === "press") await page.locator(step.selector).press(step.key);
      if (step.action === "select") await page.locator(step.selector).selectOption(step.value);
      if (step.action === "check") await page.locator(step.selector).check();
      if (step.action === "uncheck") await page.locator(step.selector).uncheck();
      if (step.action === "submit") await page.locator(step.selector).evaluate(element => {
        if (!(element instanceof HTMLFormElement)) throw new Error("Submit selector must identify a form");
        element.requestSubmit();
      });
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
export function feedbackReadPlugin({ snapshot, runtimeInputs, coverage, generatedModules = new Map() }) {
  const virtual = "\0solid-checker-feedback-entry", publicId = "virtual:solid-checker-feedback-entry";
  return {
    name: "solid-checker-feedback-reads", enforce: "pre",
    resolveId(id) { if (id === publicId) return virtual; },
    load(id) {
      if (id !== virtual) return;
      // Each document reports its start and, when unloaded, its state: a full
      // page load would otherwise discard everything recorded before it.
      // Chromium delivers a binding call from beforeunload but not pagehide.
      // Reads during unload itself are not reported.
      return `import {reads} from ${JSON.stringify("/@fs" + runtimePath)};
        import * as Solid from 'solid-js';
        globalThis.__solidCheckerDocument=crypto.randomUUID();
        Solid.OBSERVE?.diagnostics?.subscribe(event=>reads.diagnostic(event));
        globalThis.__solidCheckerFlush?.(JSON.stringify({phase:'start',id:globalThis.__solidCheckerDocument}));
        addEventListener('beforeunload',()=>globalThis.__solidCheckerFlush?.(JSON.stringify({phase:'end',
          id:globalThis.__solidCheckerDocument,state:{events:reads.events,stats:reads.stats,scopes:reads.scopes,
          diagnostics:reads.diagnostics}})));`;
    },
    transformIndexHtml() { return [{ tag: "script", attrs: { type: "module", src: "/@id/" + publicId }, injectTo: "head-prepend" }]; },
    transform(code, id, options) {
      // Middleware-generated shells may bypass transformIndexHtml. Subscribe
      // through their actual config-loaded client modules instead; this adds
      // only collection, never a replacement HTML entry or mount function.
      if (generatedModules.has(id) && !isAbsolute(id) && !options?.ssr && this.environment?.config?.consumer !== "server") {
        coverage.generatedBootstrap ??= [];
        if (!coverage.generatedBootstrap.includes(id)) coverage.generatedBootstrap.push(id);
        return { code: `import ${JSON.stringify(publicId)};\n${code}`, map: null };
      }
      const path = id.split("?")[0];
      if (!path.endsWith("/dist/dev-shared.js")) return null;
      // Resolve package identity from this exact module, including pnpm paths.
      const metadata = join(dirname(dirname(path)), "package.json");
      if (!existsSync(metadata)) return null;
      const pkg = JSON.parse(readFileSync(metadata, "utf8"));
      if (pkg.name !== "@solidjs/signals") return null;
      assert(Object.hasOwn(readerProfiles, pkg.version), "Native read collector needs a reviewed runtime profile");
      assert.equal(readFileSync(path, "utf8"), code, "A previous transform changed the native reader");
      const result = instrumentFeedbackReads(code, path, "/@fs" + runtimePath);
      runtimeInputs.set(realpathSync(metadata), { path: realpathSync(metadata), sha256: hash(readFileSync(metadata)) });
      coverage.nativeReader = { path: realpathSync(path), sha256: readerProfiles[pkg.version].sha256, version: pkg.version };
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

// Combines the collector states of every document the scenario loaded, in
// load order. Events keep their document index; counts are summed. A document
// that started but never reported its state is counted as lost.
export function mergeDocumentStates(messages, final) {
  const started = messages.filter(row => row.phase === "start").map(row => row.id);
  const ended = new Map(messages.filter(row => row.phase === "end").map(row => [row.id, row.state]));
  const states = started.map(id => id === final?.id ? final.state : ended.get(id) ?? null);
  if (final && !started.includes(final.id)) states.push(final.state);
  const present = states.filter(Boolean);
  const stats = { reads: 0, observerQueries: 0, retained: 0, dropped: 0, bytes: 0, complete: false };
  const scopes = new Map();
  let scopesDropped = 0;
  for (const state of present) {
    for (const key of ["reads", "observerQueries", "retained", "dropped", "bytes"]) stats[key] += state.stats?.[key] ?? 0;
    scopesDropped += state.scopes?.dropped ?? 0;
    for (const row of state.scopes?.rows ?? []) {
      const id = `${row.kind}:${row.path}:${row.span.start}:${row.span.end}`, previous = scopes.get(id);
      if (!previous) { scopes.set(id, { ...row }); continue; }
      for (const key of ["entered", "returned", "threw"]) if (key in row) previous[key] = (previous[key] ?? 0) + row[key];
    }
  }
  return { events: states.flatMap((state, document) => (state?.events ?? []).map(event => ({ ...event, document }))),
    stats, scopes: { rows: [...scopes.values()], dropped: scopesDropped },
    diagnostics: states.flatMap((state, document) => (state?.diagnostics ?? []).map(row => ({ ...row, document }))),
    documents: { loaded: states.length, reported: present.length, lost: states.length - present.length } };
}

async function collectProject(project, scenario, tools, browser, responses = new Map(), runtimeOptions = {}) {
  const { snapshot, leaves } = feedbackBrowserSnapshots(project), projectRoot = dirname(snapshot.manifest.project);
  // This run executes the application as a client-rendered development build
  // in Chromium, which is the runtime the native analysis answers for.
  const nativeOptions = { feedbackFacts: true, runtime: { target: "browser", build: "development", rendering: "csr" } };
  const projects = leaves?.map(leaf => {
    const report = inspectDevelopmentFeedback(leaf.manifest.project, { ...nativeOptions, snapshot: leaf });
    assert(Array.isArray(report.analysis.feedbackFacts), `Native checker does not expose development models: ${leaf.manifest.project}`);
    return { project: leaf.manifest.project, report };
  });
  const nativeFeedback = projects ? inspectDevelopmentFeedback(project, { snapshot, analyze: () => ({
    status: Math.max(...projects.map(row => row.report.nativeExitCode)), stdout: JSON.stringify({
      findings: projects.flatMap(row => row.report.analysis.findings),
      feedbackFacts: projects.flatMap(row => row.report.analysis.feedbackFacts) }) }) })
    : inspectDevelopmentFeedback(project, { ...nativeOptions, snapshot });
  assert(Array.isArray(nativeFeedback.analysis.feedbackFacts), "Native checker does not expose development models");
  const { models, conflictingLeafModels } = selectFeedbackModels(nativeFeedback.analysis.feedbackFacts);
  const runtimeInputs = new Map(), coverage = { nativeReader: null, sourceInstrumentation: [], complete: false,
    ...(conflictingLeafModels.size ? { conflictingLeafModels: [...conflictingLeafModels] } : {}) }, unmapped = [];
  const instrumented = new Map();
  const sourceFiles = new Map(snapshot.sources.filter(source => !source.isDeclarationFile)
    .map(source => [resolve(source.fileName), source]));
  const record = path => {
    path = realpathSync(path);
    const pin = { path, sha256: hash(readFileSync(path)) }, previous = runtimeInputs.get(path);
    assert(!previous || previous.sha256 === pin.sha256, `Runtime input changed: ${path}`);
    runtimeInputs.set(path, pin);
  };
  const serverDiagnostics = createFeedbackDiagnostics();
  for (const path of runtimeOptions.boundaryInputs ?? []) record(path);
  const configuration = await loadFeedbackConfiguration(tools, projectRoot, record, runtimeOptions, serverDiagnostics.logger), root = realpathSync(configuration.root);
  configuration.configuration.root = root;
  configuration.configuration.options = { root: runtimeOptions.root ?? null, config: runtimeOptions.config ?? null };
  const physicalEntry = existsSync(join(root, "index.html"));
  assert(physicalEntry || configuration.configuration.kind === "application-vite-config", "Browser feedback needs an application HTML entry; no index.html or application config exists");
  record(runtimePath);
  if (physicalEntry) record(join(root, "index.html"));
  else recordFeedbackGeneratorInputs(configuration.dependencies, record);
  const provenance = physicalEntry ? null : await observeFeedbackEntry(configuration.config.plugins);
  const application = { ...configuration.config, plugins: provenance?.plugins ?? configuration.config.plugins };
  const cache = mkdtempSync(join(tmpdir(), "solid-feedback-vite-"));
  let server, context;
  const pageErrors = [], blockedRequests = [], consoleDiagnostics = [];
  let consoleDiagnosticsDropped = 0;
  try {
    server = await tools.createServer(mergeFeedbackServerConfiguration(tools, application, { configFile: false, root, cacheDir: cache,
    plugins: [...(provenance ? [provenance.observer] : []),
      feedbackReadPlugin({ snapshot, runtimeInputs, coverage, generatedModules: provenance?.modules }), {
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
    customLogger: serverDiagnostics.logger, logLevel: "silent" }));
    await server.listen();
    const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
    context = await browser.newContext(feedbackContextOptions(scenario)); context.setDefaultTimeout(12000);
    await context.route("**/*", route => routeFeedbackRequest(route, origin, responses, blockedRequests));
    const page = await context.newPage(), documentMessages = [];
    // A raw CDP binding: Playwright's exposed bindings drop calls from a
    // document that is already unloading, which is exactly the flush needed.
    const cdp = await context.newCDPSession(page);
    cdp.on("Runtime.bindingCalled", ({ name, payload }) => {
      if (name !== "__solidCheckerFlush") return;
      try { documentMessages.push(JSON.parse(payload)); } catch { documentMessages.push({ phase: "malformed" }); }
    });
    await cdp.send("Runtime.enable");
    await cdp.send("Runtime.addBinding", { name: "__solidCheckerFlush" });
    page.on("pageerror", error => pageErrors.push({ message: error.message, stack: error.stack }));
    page.on("response", response => serverDiagnostics.response(response, origin));
    page.on("console", message => {
      if (!["error", "warning"].includes(message.type())) return;
      if (consoleDiagnostics.length >= 256) { consoleDiagnosticsDropped++; return; }
      consoleDiagnostics.push({ severity: message.type(), message: message.text(), location: message.location() });
    });
    const entryResponse = await page.goto(new URL(server.config.base, origin).href, { waitUntil: "domcontentloaded" });
    const entry = physicalEntry ? { kind: "physical-index", path: realpathSync(join(root, "index.html")) }
      : await admitFeedbackEntry({ response: entryResponse, scripts: await page.evaluate(html =>
        [...new DOMParser().parseFromString(html, "text/html").querySelectorAll('script[type="module"][src]')].map(script => script.getAttribute("src")),
        (await entryResponse.body()).toString()),
        origin, appTree: runtimeOptions.appTree ?? root, server, provenance, record });
    const { assertions, checkpoints, failure } = await executeFeedbackScenario(page, scenario);
    const final = await page.evaluate(() => globalThis.__solidCheckerDocument ? { id: globalThis.__solidCheckerDocument,
      state: { events: globalThis.__solidCheckerReads?.events ?? [], stats: globalThis.__solidCheckerReads?.stats ?? null,
        scopes: globalThis.__solidCheckerReads?.scopes ?? { rows: [], dropped: 0 }, diagnostics: globalThis.__solidCheckerReads?.diagnostics ?? [] } } : null);
    const raw = mergeDocumentStates(documentMessages, final);
    coverage.documents = raw.documents;
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
        attributions.push(row);
        if (row.outcome === "mapped") return { site: site(row), attributions };
      }
      return { unmapped: classifyUnmappedFrames(frames, attributions, { stackTruncated }), attributions };
    }
    const authoredEvents = [];
    for (const [index, event] of raw.events.entries()) {
      const located = coverage.nativeReader ? await locate(event.frames, event.stackTruncated ?? null) :
        { unmapped: { attribution: "no-reviewed-reader", reason: "The reviewed native reader was not instrumented", frames: event.frames } };
      if (!located.site) { unmapped.push({ kind: event.kind ?? "untracked-read", nodeId: event.nodeId, document: event.document, ...located.unmapped }); continue; }
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
    // A Solid dev diagnostic is the runtime's own report. Its site is the first
    // frame that maps to configured source; otherwise it keeps its frames and
    // attribution class, exactly as an unmapped read does.
    const diagnostics = [];
    for (const row of raw.diagnostics) {
      const located = await locate(row.frames ?? [], row.stackTruncated ?? null);
      diagnostics.push({ code: row.code, kind: row.kind, severity: row.severity, message: row.message, document: row.document,
        channel: "runtime-diagnostic", authority: false, certification: false, operation: operationFrame(located.attributions),
        ...(located.site ? { siteKind: siteExpressionKind(sourceFiles.get(located.site.location.path).text, located.site.location.path,
          Buffer.from(sourceFiles.get(located.site.location.path).text).subarray(0, located.site.location.startByte).toString().length) } : {}),
        ...(located.site ? { site: located.site } : { site: null, attribution: located.unmapped.attribution,
          firstPackageFrame: located.unmapped.firstPackageFrame, packages: located.unmapped.packages }) });
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
    if (projects) { feedback.solution = { referencedProjects: projects.map(row => row.project) }; feedback.projects = projects; }
    const automatic = selectReadFeedback(authoredEvents, [...models.values()], { typingErrors: snapshot.typingErrors, dropped: raw.stats?.dropped ?? 0 });
    if (raw.documents.lost) automatic.open?.push({ reason: "A loaded document unloaded without reporting its records", lost: raw.documents.lost });
    return { feedback, capture, assertions: snapshot.typingErrors ? [] : assertions,
      automatic,
      coverage, stats: raw.stats, diagnostics, checkpoints, unmapped, blockedRequests,
      suppliedResponses: [...responses.values()].map(row => ({ url: row.url, method: row.method,
        ...(row.requestBody !== undefined ? { requestBody: row.requestBody, requestBodySha256: row.requestBodySha256 } : {}), status: row.status, contentType: row.contentType,
        body: row.pin, served: row.served, channel: "scenario-input" })),
      pageErrors, consoleDiagnostics, consoleDiagnosticsDropped, serverDiagnostics: await serverDiagnostics.finish(), failure,
      entry, runtimeInputs: capture.runtimeInputs, ...(scenario.viewport ? { viewport: scenario.viewport } : {}),
      configuration: configuration.configuration, typingErrors: snapshot.typingErrors };
  } finally { await closeFeedbackResources({ context, server, cache }); }
}

export function selectFeedbackModels(rows) {
  const models = new Map(), sourceHashes = new Map(), conflictingLeafModels = new Set();
  for (const model of rows) {
    assert(!sourceHashes.has(model.path) || sourceHashes.get(model.path) === model.sourceSha256, `Leaf feedback source changed: ${model.path}`);
    sourceHashes.set(model.path, model.sourceSha256);
    if (conflictingLeafModels.has(model.path)) continue;
    const previous = models.get(model.path);
    if (previous && JSON.stringify(previous) !== JSON.stringify(model)) {
      conflictingLeafModels.add(model.path); models.delete(model.path); continue;
    }
    models.set(model.path, model);
  }
  return { models, conflictingLeafModels };
}

export async function runBrowserFeedback({ project, scenarioPath, browserPath, toolingRoot, comparisonProject, root, config }) {
  const scenarioPin = { path: realpathSync(resolve(scenarioPath)), sha256: hash(readFileSync(scenarioPath)) };
  const scenario = validateFeedbackScenario(JSON.parse(readFileSync(scenarioPin.path, "utf8")));
  const responses = loadSuppliedResponses(scenario, scenarioPin.path);
  const runtimeOptions = validateFeedbackRuntimeOptions(project, { root, config });
  assert(!comparisonProject || (!root && !config), "Explicit --root/--config with --compare-project needs separate app inputs; run each app separately");
  const tools = await toolingAt(resolve(toolingRoot ?? dirname(resolve(project))));
  tools.root = realpathSync(resolve(toolingRoot ?? dirname(resolve(project))));
  const browserPin = { path: realpathSync(resolve(browserPath)), sha256: hash(readFileSync(browserPath)) };
  const browser = await tools.chromium.launch({ executablePath: browserPin.path, headless: true, timeout: 15000 });
  try {
    const fresh = () => new Map([...responses].map(([url, row]) => [url, { ...row, served: 0 }]));
    const original = await collectProject(project, scenario, tools, browser, fresh(), runtimeOptions);
    const comparison = comparisonProject ? await collectProject(comparisonProject, scenario, tools, browser, fresh(), validateFeedbackRuntimeOptions(comparisonProject)) : null;
    for (const run of [original, comparison].filter(Boolean)) {
      validateFeedbackInputs(run.capture.manifest);
      for (const pin of run.capture.runtimeInputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256, `Runtime input changed: ${pin.path}`);
    }
    assert.equal(hash(readFileSync(scenarioPin.path)), scenarioPin.sha256, "Scenario changed during execution");
    for (const { pin } of responses.values()) assert.equal(hash(readFileSync(pin.path)), pin.sha256, `Supplied response changed: ${pin.path}`);
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
  } finally { await closeFeedbackResources({ browser }); }
}
