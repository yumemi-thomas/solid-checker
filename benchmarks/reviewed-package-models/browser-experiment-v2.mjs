// Execute retained packages in a real browser. Outputs are observations, not
// accepted contracts; unexpected diagnostics and thrown exceptions are kept.
import assert from "node:assert/strict";
import { cpSync, existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from "./catalog.mjs";
import { ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
import holdouts from "./holdout-cases.mjs";
import { guardTracePlugin } from "./guard-trace.mjs";
import { originTracePlugin } from "./origin-trace.mjs";

const [outArgument, executablePath, filterArgument = ""] = process.argv.slice(2);
const extraCases = process.env.REVIEWED_MODEL_BROWSER_CASES ? (await import(pathToFileURL(resolve(process.env.REVIEWED_MODEL_BROWSER_CASES)))).default : [];
const sourcePluginFactory = process.env.REVIEWED_MODEL_SOURCE_PLUGIN ? (await import(pathToFileURL(resolve(process.env.REVIEWED_MODEL_SOURCE_PLUGIN)))).default : null;
const filter = filterArgument || (extraCases.length ? extraCases.map(entry => entry.id).join(',') : '');
assert(outArgument && executablePath, "Usage: node browser-experiment-v2.mjs <fresh-output-directory> <browser-executable>");
const out = resolve(outArgument); assert(!existsSync(out)); mkdirSync(out, { recursive: true });
const repo = resolve(new URL("../..", import.meta.url).pathname);
const apps = join(repo, "rust/target/app-import-metric/apps"), tooling = join(apps, "helge-dev");
const load = async (name, entry) => import(pathToFileURL(join(packageRoot(tooling, name), entry)));
const { createServer } = await load("vite", "dist/node/index.js");
const { default: solid } = await load("@solidjs/vite-plugin", "dist/esm/index.mjs");
const { chromium } = await load("playwright", "index.mjs");
const { TraceMap, originalPositionFor } = await load("@jridgewell/trace-mapping", "dist/trace-mapping.mjs");
const retained = read(join(repo, "rust/target/primitives-checkpoint/run-browser.json"));
const bridge = process.env.REVIEWED_MODEL_FEEDBACK_BRIDGE ? resolve(process.env.REVIEWED_MODEL_FEEDBACK_BRIDGE) : resolve(new URL("./runtime-feedback.mjs", import.meta.url).pathname);
const report = { authority: false, basis: "browser-runtime-observation", startedAt: new Date().toISOString(),
  browserExecutable: executablePath, bridgeSha256: hash(readFileSync(bridge)),
  tooling: ["vite", "@solidjs/vite-plugin", "playwright"].map(name => ({ package: name, version: read(join(packageRoot(tooling, name), "package.json")).version })), results: [] };
const save = () => writeFileSync(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
const browser = await chromium.launch({ executablePath, headless: true, timeout: 15000 }); report.browser = browser.version();
function typing(root, path) {
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions("v2", true, { customConditions: ["browser", "development"] }), allowJs: true }, root).options;
  const ambient = join(root, "src/vite-env.d.ts");
  const errors = ts.getPreEmitDiagnostics(ts.createProgram([path, ...(existsSync(ambient) ? [ambient] : [])], options)).filter(d => d.category === ts.DiagnosticCategory.Error);
  return errors.map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, "\n"), file: d.file?.fileName, start: d.start }));
}
async function execute(id, install, source, flow, { appCopy = false, packages = [], provenance = null, patchCopy = null } = {}) {
  if (filter && !filter.split(",").includes(id)) return;
  const root = join(out, id); mkdirSync(root);
  if (appCopy) for (const name of ["src", "static", "index.html", "tsconfig.json", "package.json"]) cpSync(join(install, name), join(root, name), { recursive: true });
  else { mkdirSync(join(root, "src")); writeFileSync(join(root, "index.html"), '<div id="root"></div><script type="module" src="/src/main.tsx"></script>'); writeFileSync(join(root, "src/main.tsx"), source); }
  const patchEvidence = patchCopy?.(root) ?? null;
  symlinkSync(join(install, "node_modules"), join(root, "node_modules"), "dir");
  const main = join(root, appCopy ? "src/index.tsx" : "src/main.tsx");
  const originalSourceSha256 = hash(readFileSync(main));
  if (appCopy) {
    const text = readFileSync(main, 'utf8'), mount = 'render(() => <App />, root);';
    assert(text.includes(mount)); writeFileSync(main, text.replace(mount, '(globalThis as any).__experiment.dispose = ' + mount));
  }
  const item = { id, provenance, sourceSha256: hash(readFileSync(main)),
    originalSourceSha256, mountResultExposedForDisposal: appCopy, patchEvidence,
    runtime: nativeRuntimeRoots(root).map(dir => ({ ...read(join(dir, "package.json")), dir })).map(({ name, version, dir }) => ({ name, version, dir })),
    packagePins: packages.map(name => ({ package: name, pins: closurePins(packageRoot(root, name)) })),
    publishedTypingErrors: typing(root, main), steps: [], pageErrors: [], consoleErrors: [], blockedRequests: [], feedback: [] };
  // A type-invalid target cannot support this checker's semantic claim.
  if (item.publishedTypingErrors.length && extraCases.some(entry => entry.id === id)) { item.excludedBeforeExecution = true; report.results.push(item); save(); return; }
  report.results.push(item); save();
  const attributionAvailable = item.runtime.every(runtime => runtime.version === '2.0.0-rc.9') && !!read(join(packageRoot(root, 'solid-js'), 'package.json')).exports?.['./attribution'];
  const useAttribution = process.env.REVIEWED_MODEL_ATTRIBUTION === '1' && attributionAvailable;
  const prebundleAttribution = process.env.REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE === '1' && useAttribution;
  writeFileSync(join(root, "feedback-entry.mjs"), `import * as Solid from 'solid-js';
import { runtimeFeedback } from ${JSON.stringify('/@fs' + bridge)};
import { collectGuardTrace } from ${JSON.stringify('/@fs' + resolve(new URL('./guard-trace-runtime-v2.mjs', import.meta.url).pathname))};
import { collectPackageOrigins } from ${JSON.stringify('/@fs' + resolve(new URL('./origin-trace-runtime.mjs', import.meta.url).pathname))};
${useAttribution ? "import { attribution } from 'solid-js/attribution'; attribution.enable({ log: false, stacks: true });" : ''}
const harness = globalThis.__experiment = { feedback: [], observations: [], errors: [], values: {}, disposals: 0 };
globalThis.__packageOrigins = collectPackageOrigins(); harness.originFailures = globalThis.__packageOrigins.failures;
const errorIds = new WeakMap(); let nextErrorId = 0; harness.windowErrors = [];
window.addEventListener('error', event => { const error = event.error; let errorId = null; if (error && typeof error === 'object') { if (!errorIds.has(error)) errorIds.set(error, ++nextErrorId); errorId = errorIds.get(error); } harness.windowErrors.push({ errorId, message: error?.message ?? event.message, stack: error?.stack ?? null }); });
${useAttribution ? "harness.attributionProbe = () => ({ installed: !!Solid.OBSERVE?.attribution?.installed, runs: attribution.history().map(({ nodeName, nodeKind, changed, causes }) => ({ nodeName, nodeKind, changed, causes: causes.map(({ kind, name, origin }) => ({ kind, name, origin: origin?.kind })) })) });" : ''}
globalThis.__solidGuardTrace = collectGuardTrace(frame => new URL(frame.path).pathname.startsWith('/src/')); harness.guardTrace = globalThis.__solidGuardTrace.events;
harness.diagnosticSubscription = !!Solid.OBSERVE?.diagnostics?.subscribe;
if (harness.diagnosticSubscription) harness.observer = runtimeFeedback(Solid.OBSERVE, { isAppFrame: frame => new URL(frame.path).pathname.startsWith('/src/'), onFeedback: value => harness.feedback.push(value) });
harness.attempt = (label, fn) => { try { const value = fn(); harness.observations.push({ label, ok: true }); return value; } catch (error) { harness.errors.push({ label, message: error.message, stack: error.stack }); } };
Solid.OBSERVE?.diagnostics?.subscribe(event => { harness.observations.push({ channelCode: event.code, kind: event.kind, severity: event.severity }); });`);
  const guardPlugin = process.env.REVIEWED_MODEL_GUARD_TRACE === '1' ? guardTracePlugin() : null;
  const originPlugin = process.env.REVIEWED_MODEL_ORIGIN_TRACE === '1' ? originTracePlugin() : null;
  const sourcePlugin = sourcePluginFactory?.();
  assert(!(guardPlugin && originPlugin), 'Run the two source instrumentation profiles separately');
  assert(!prebundleAttribution || !guardPlugin && !originPlugin, 'Original-file traces are separate from the prebundled attribution profile');
  const server = await createServer({ configFile: false, root, cacheDir: join(root, ".vite-cache"), publicDir: appCopy ? "static" : false,
    plugins: [...(guardPlugin ? [guardPlugin] : []), ...(originPlugin ? [originPlugin] : []), ...(sourcePlugin ? [sourcePlugin] : []), { name: "experiment-observer", transformIndexHtml: { order: "pre", handler() { return [{ tag: "script", attrs: { type: "module", src: "/feedback-entry.mjs" }, injectTo: "head-prepend" }]; } } }, solid({ hot: false })],
    resolve: { alias: { "solid-js/web": "@solidjs/web" }, dedupe: ["solid-js", "@solidjs/signals", "@solidjs/web"] },
    server: { host: "127.0.0.1", port: 0, fs: { allow: [repo, install, root, dirname(bridge)] } },
    // The attribution subpath shares live dev-shared state with the core.
    // Prebundling only the core splits that state from a separately loaded
    // attribution module and also hides source instrumentation inputs.
    optimizeDeps: { noDiscovery: true, include: prebundleAttribution ? ['solid-js', 'solid-js/attribution', '@solidjs/signals', '@solidjs/signals/attribution', '@solidjs/web'] : [], ...(!prebundleAttribution && (process.env.REVIEWED_MODEL_ATTRIBUTION === '1' || originPlugin || process.env.REVIEWED_MODEL_DISABLE_CORE_PREBUNDLE === '1') ? {
      exclude: ['solid-js', 'solid-js/attribution', '@solidjs/signals', '@solidjs/signals/attribution', '@solidjs/web'],
    } : {}) }, logLevel: "error" });
  let context;
  try {
    await server.listen(); const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
    context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    await context.route("**/*", route => { const url = new URL(route.request().url()); if (url.origin === origin) return route.continue(); item.blockedRequests.push(url.href); return route.abort(); });
    context.setDefaultTimeout(8000);
    const page = await context.newPage(); page.on("pageerror", error => item.pageErrors.push({ message: error.message, stack: error.stack }));
    page.on("console", event => { if (event.type() === "error") item.consoleErrors.push(event.text()); });
    const step = async (label, fn) => { const start = performance.now(); await fn(); item.steps.push({ label, durationMs: performance.now() - start }); save(); };
    await step("mount", () => page.goto(origin, { waitUntil: "networkidle" }));
    try { await flow(page, step); } catch (error) { item.harnessFailure = { message: error.message, stack: error.stack }; }
    const state = await page.evaluate(() => { const { feedback, observations, errors, values, disposals, diagnosticSubscription, guardTrace, originFailures, attributionProbe, windowErrors } = globalThis.__experiment; return { feedback, observations, errors, values, disposals, diagnosticSubscription, guardTrace, originFailures, windowErrors, attributionState: attributionProbe?.() ?? null }; });
    Object.assign(item, state);
    if (sourcePlugin) item.sourceInstrumentation = { transformed: sourcePlugin.transformed, refused: sourcePlugin.refused };
    if (guardPlugin) item.guardInstrumentation = { transformed: guardPlugin.transformed, refused: guardPlugin.refused,
      instrumenterSha256: hash(readFileSync(new URL('./guard-trace.mjs', import.meta.url))), collectorSha256: hash(readFileSync(new URL('./guard-trace-runtime-v2.mjs', import.meta.url))) };
    item.attributionRequested = process.env.REVIEWED_MODEL_ATTRIBUTION === '1';
    item.attributionPrebundleRequested = process.env.REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE === '1';
    item.attributionPrebundle = prebundleAttribution;
    item.attributionEnabled = item.attributionState?.installed ?? false;
    if (originPlugin) item.originInstrumentation = { transformed: originPlugin.transformed, refused: originPlugin.refused,
      instrumenterSha256: hash(readFileSync(new URL('./origin-trace.mjs', import.meta.url))), collectorSha256: hash(readFileSync(new URL('./origin-trace-runtime.mjs', import.meta.url))) };
    const maps = new Map();
    async function remap(frame) {
      const url = new URL(frame.path); if (url.origin !== origin) return null;
      const path = url.pathname + url.search;
      if (!maps.has(path)) { try { const transformed = await server.transformRequest(path); maps.set(path, transformed?.map ? new TraceMap(transformed.map) : null); } catch { maps.set(path, null); } }
      const map = maps.get(path); if (!map) return null;
      const location = originalPositionFor(map, { line: frame.line, column: frame.column - 1 });
      const servedPath = url.pathname.startsWith('/@fs/') ? decodeURIComponent(url.pathname.slice(4)) : join(root, decodeURIComponent(url.pathname));
      return location.source && location.line ? { path: resolve(dirname(servedPath), location.source), line: location.line, column: location.column + 1, name: location.name } : null;
    }
    for (const feedback of item.feedback) {
      feedback.originalLocation = feedback.location ? await remap(feedback.location) : null;
      const registrationFrame = feedback.registration?.frames.find(frame => new URL(frame.path).pathname.startsWith('/src/'));
      if (registrationFrame) feedback.originalRegistration = await remap(registrationFrame);
      feedback.originalFrames = await Promise.all(feedback.frames.map(remap));
      const appIndex = feedback.frames.findIndex(frame => new URL(frame.path).pathname.startsWith("/src/"));
      feedback.dependencyOperation = feedback.frames.slice(0, appIndex < 0 ? undefined : appIndex).filter(frame => frame.path.includes("/node_modules/")).at(-1) ?? null;
      // A dependency operation is evidence about the origin, not a verdict
      // that the app call is wrong. Returned accessor stacks may lose origin.
      feedback.attribution = feedback.dependencyOperation ? "dependency-frame-before-app-entry" : "app-frame-without-package-origin";
    }
    for (const note of item.guardTrace ?? []) {
      note.originalLocation = note.location ? await remap(note.location) : null;
      note.originalFrames = await Promise.all(note.frames.map(async frame => {
        const mapped = await remap(frame);
        return mapped && existsSync(mapped.path) ? { ...mapped, sourceSha256: hash(readFileSync(mapped.path)) } : null;
      }));
    }
    for (const failure of item.originFailures) {
      const frame = failure.registration.frames.find(frame => new URL(frame.path).pathname.startsWith('/src/'));
      failure.originalRegistration = frame ? await remap(frame) : null;
    }
    for (const error of [...item.errors, ...item.pageErrors]) {
      const frames = (error.stack ?? '').split('\n').flatMap(line => { const match = line.match(/(?:at .*?\()?((?:https?:\/\/).*?):(\d+):(\d+)\)?$/); return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : []; });
      const frame = frames.find(frame => new URL(frame.path).pathname.startsWith('/src/'));
      error.originalLocation = frame ? await remap(frame) : null;
    }
    await page.screenshot({ path: join(root, "final.png"), fullPage: true });
  } catch (error) { item.harnessFailure = { message: error.message, stack: error.stack }; }
  finally { await context?.close(); await server.close(); save(); }
}
const appImportMetric = read(join(repo, "rust/target/app-import-metric/metric.json"));
try {
  await execute("helge-app", tooling, null, async (page, step) => {
    await step("home", () => page.getByRole("heading", { name: "Helge Falch", exact: true }).waitFor());
    for (const label of ["Projects", "About", "Home"]) await step(`navigate-${label}`, async () => { await page.locator(".innerContainer .buttons").getByRole("link", { name: label, exact: true }).click(); await page.waitForURL(label === "Home" ? "**/" : `**/${label.toLowerCase()}`); });
    await step("modal-open-close", async () => { await page.locator(".icons [role=button]").click(); await page.locator(".modal").waitFor(); await page.locator(".backdrop").click({ position: { x: 10, y: 10 } }); await page.locator(".modal").waitFor({ state: "hidden" }); });
    await step("mobile-open-navigate", async () => { await page.setViewportSize({ width: 390, height: 844 }); await page.getByRole("button", { name: "menu-burger-button" }).click(); await page.locator(".responsiveButtons").getByRole("link", { name: "About", exact: true }).click(); await page.waitForURL("**/about"); await page.locator(".NavBar.open").waitFor({ state: "hidden" }); });
    await step("history-back", async () => { await page.goBack(); await page.waitForURL("**/"); });
    await step('dispose-app', async () => { await page.evaluate(() => { globalThis.__experiment.dispose(); globalThis.__experiment.disposals++; }); assert.equal(await page.locator('#root').textContent(), ''); });
  }, { appCopy: true, packages: ["@solidjs/router", "@solidjs/meta", "solid-icons"], provenance: { cachedApp: "helge-dev", metric: appImportMetric.generatedAt ?? null, adaptedCachedSource: true } });

  const routerCallPath = join(tooling, "src/components/NavBar.tsx"), routerSource = ts.createSourceFile(routerCallPath, readFileSync(routerCallPath, "utf8"), ts.ScriptTarget.Latest, true);
  // Resolve the exact imported symbol before reusing the zero-argument call.
  const routerProgram = ts.createProgram([routerCallPath], ts.convertCompilerOptionsFromJson(oracleCompilerOptions("v2", true), tooling).options);
  const routerChecker = routerProgram.getTypeChecker(), bound = routerProgram.getSourceFile(routerCallPath);
  let routerCall;
  function visit(node) { if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) { const symbol = routerChecker.getSymbolAtLocation(node.expression); const target = symbol?.flags & ts.SymbolFlags.Alias ? routerChecker.getAliasedSymbol(symbol) : symbol; if (target?.getName() === "useLocation" && target.declarations?.some(d => d.getSourceFile().fileName.includes("@solidjs/router/"))) routerCall = node; } ts.forEachChild(node, visit); }
  visit(bound); assert(routerCall && routerCall.arguments.length === 0);
  const callText = routerCall.getText(bound), position = bound.getLineAndCharacterOfPosition(routerCall.getStart());
  for (const phase of ["setup", "memo", "effect", "event"]) {
    const operation = `const location = ${callText}; harness.location = location; harness.values.pathname = location.pathname;`;
    const body = phase === "setup" ? `harness.attempt('${phase}', () => { ${operation} });` :
      phase === "memo" ? `const value = createMemo(() => { ${operation} return location.pathname; }); harness.attempt('${phase}', () => createEffect(value, value => { harness.values.pathname = value; }));` :
      phase === "effect" ? `createEffect(() => 1, () => { harness.attempt('${phase}', () => { ${operation} }); });` :
      `harness.event = () => harness.attempt('${phase}', () => { ${operation} });`;
    await execute(`router-${phase}`, tooling, `import { createMemo, createEffect } from 'solid-js'; import { render } from '@solidjs/web'; import { createRouter, useLocation } from '@solidjs/router';
const harness = (globalThis as any).__experiment;
function Probe() { ${body} return <div><a href='/next'>next</a><p>router probe</p></div>; }
const Router = createRouter({ routes: [{ path: '/', component: Probe }, { path: '/next', component: Probe }] });
harness.dispose = render(() => <Router />, document.getElementById('root')!);`, async (page, step) => {
      if (phase === "event") await step("invoke-event", () => page.evaluate(() => globalThis.__experiment.event()));
      await step("navigate", async () => { await page.getByRole("link", { name: "next", exact: true }).click(); await page.waitForURL("**/next"); });
      await step("dispose", () => page.evaluate(() => { globalThis.__experiment.dispose(); globalThis.__experiment.disposals++; }));
    }, { packages: ["@solidjs/router"], provenance: { path: routerCallPath, line: position.line + 1, callText, sourceSha256: hash(routerSource.text), argumentsCopied: true } });
  }
  const primitivePrelude = `import { createRoot, createComponent, createEffect, createMemo, createSignal, flush } from 'solid-js';
const harness = (globalThis as any).__experiment;`;
  for (const phase of ["module", "setup", "memo", "effect", "event"]) {
    const row = retained.results.find(row => row.package === "@solid-primitives/timer");
    const call = "createTimer(() => { harness.values.ticks = (harness.values.ticks ?? 0) + 1; }, 10, setInterval)";
    const body = phase === "module" ? `${call};` : phase === "setup" ? `${call};` : phase === "memo" ? `const value = createMemo(() => { ${call}; return 1; }); createEffect(value, () => {});` : phase === "effect" ? `createEffect(() => 1, () => { ${call}; });` : `harness.event = () => { ${call}; };`;
    await execute(`timer-${phase}`, row.retainedArtifacts.projectDir, `${primitivePrelude} import { createTimer } from '@solid-primitives/timer';
${phase === "module" ? body : ""}
harness.dispose = createRoot(dispose => { createComponent(function Probe() { ${phase === "module" ? "" : body} return null; }, {}); return dispose; }); flush();`, async (page, step) => {
      if (phase === "event") await step("invoke-event", () => page.evaluate(() => globalThis.__experiment.event()));
      await step("wait-for-ticks", () => page.waitForFunction(() => globalThis.__experiment.values.ticks >= 2));
      await step("dispose", () => page.evaluate(() => { const h = globalThis.__experiment; h.dispose(); h.disposals++; h.values.ticksAtDispose = h.values.ticks; }));
      await page.waitForTimeout(60); await page.evaluate(() => { const h = globalThis.__experiment; h.values.ticksAfterDispose = h.values.ticks; });
    }, { packages: [row.package], provenance: { syntheticCall: true, argumentProfile: "numeric delay + setInterval" } });
  }
  for (const eager of [false, true]) {
    const row = retained.results.find(row => row.package === "@solid-primitives/pagination");
    await execute(`pagination-${eager ? "eager" : "tracked"}`, row.retainedArtifacts.projectDir, `${primitivePrelude} import { createPagination } from '@solid-primitives/pagination';
harness.dispose = createRoot(dispose => { createComponent(function Probe() { const [, page, setPage] = createPagination({ pages: 10 });
${eager ? "harness.values.page = page();" : "createEffect(() => page(), value => { harness.values.page = value; });"}
harness.update = () => { setPage(2); flush(); }; return null; }, {}); return dispose; }); flush();`, async (page, step) => {
      await step("update", () => page.evaluate(() => globalThis.__experiment.update()));
      await step("dispose", () => page.evaluate(() => { globalThis.__experiment.dispose(); globalThis.__experiment.disposals++; }));
    }, { packages: [row.package], provenance: { syntheticCall: true } });
  }
  await execute("corvu-dialog", join(apps, "sefer"), `import Dialog from '@corvu-next/dialog'; import { render } from '@solidjs/web';
const harness = (globalThis as any).__experiment;
harness.dispose = render(() => <Dialog><Dialog.Trigger>open dialog</Dialog.Trigger><Dialog.Portal><Dialog.Content><Dialog.Label>Browser dialog</Dialog.Label><Dialog.Description>Local test</Dialog.Description><Dialog.Close>close dialog</Dialog.Close></Dialog.Content></Dialog.Portal></Dialog>, document.getElementById('root')!);`, async (page, step) => {
    await step("open-close", async () => { await page.getByRole("button", { name: "open dialog" }).click(); await page.getByRole("dialog").waitFor(); await page.getByRole("button", { name: "close", exact: true }).click(); await page.getByRole("dialog").waitFor({ state: "hidden" }); });
    await step("escape-focus-restore", async () => { await page.getByRole("button", { name: "open dialog" }).click(); await page.getByRole("dialog").waitFor(); await page.keyboard.press("Escape"); await page.getByRole("dialog").waitFor({ state: "hidden" }); await page.waitForFunction(() => document.activeElement?.textContent === 'open dialog'); });
    await step("dispose", () => page.evaluate(() => { globalThis.__experiment.dispose(); globalThis.__experiment.disposals++; }));
  }, { packages: ["@corvu-next/dialog"], provenance: { syntheticIntegration: true, cachedApp: "sefer" } });
  await execute("query-local", join(apps, "spotify-desk-thing"), `import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/solid-query'; import { Loading } from 'solid-js'; import { render } from '@solidjs/web';
const harness = (globalThis as any).__experiment;
const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
function Probe() { const result = useQuery(() => ({ queryKey: ['local'], queryFn: async () => { harness.values.fetches = (harness.values.fetches ?? 0) + 1; return harness.values.fetches as number; } }));
harness.refetch = () => result.refetch(); return <p id='value'>{result.data}</p>; }
harness.dispose = render(() => <Loading fallback={<p>loading</p>}><QueryClientProvider client={client}><Probe /></QueryClientProvider></Loading>, document.getElementById('root')!);
harness.subscribers = () => client.getQueryCache().find({ queryKey: ['local'] })?.getObserversCount();`, async (page, step) => {
    await step("query-resolve", async () => { await page.locator("#value").filter({ hasText: "1" }).waitFor(); });
    await step("query-refetch", async () => { await page.evaluate(() => globalThis.__experiment.refetch()); await page.locator("#value").filter({ hasText: "2" }).waitFor(); });
    await step("dispose", () => page.evaluate(() => { const h = globalThis.__experiment; h.dispose(); h.disposals++; h.values.subscribersAfterDispose = h.subscribers(); }));
  }, { packages: ["@tanstack/solid-query"], provenance: { syntheticIntegration: true, cachedApp: "spotify-desk-thing", originalOptionsOpaque: true, staticRc9AuditRefused: true } });
  for (const entry of holdouts) for (const twin of ['misuse', 'correct']) {
    if (!entry[twin]) continue;
    const row = retained.results.find(row => row.package === entry.package);
    await execute(`${entry.id}-${twin}`, row.retainedArtifacts.projectDir,
      `${entry[twin]}\nimport { render } from '@solidjs/web'; const harness = (globalThis as any).__experiment; harness.dispose = render(() => <App />, document.getElementById('root')!);`,
      async (page, step) => { await step('dispose', () => page.evaluate(() => { globalThis.__experiment.dispose(); globalThis.__experiment.disposals++; })); },
      { packages: [entry.package], provenance: { frozenConsumerHoldout: true, sourceSha256: hash(entry[twin]), expectedStaticTarget: twin === 'misuse' && entry.expectWarning !== false } });
  }
  // A behavioral challenge for which intentional update expectations are
  // specified by this test. A quiet runtime cannot establish reactive UI.
  for (const snapshot of [true, false]) {
    const row = retained.results.find(row => row.package === '@solid-primitives/map');
    await execute(`map-${snapshot ? 'snapshot' : 'tracked'}-challenge`, row.retainedArtifacts.projectDir,
      `import { ReactiveMap } from '@solid-primitives/map'; import { render } from '@solidjs/web'; import { flush } from 'solid-js';
const harness = (globalThis as any).__experiment;
function App() { const map = new ReactiveMap<string, number>(); ${snapshot ? 'const size = map.size;' : ''}
harness.update = () => { map.set('key', 1); flush(); }; harness.actualSize = () => map.size;
return <p id='value'>{${snapshot ? 'size' : 'map.size'}}</p>; }
harness.dispose = render(() => <App />, document.getElementById('root')!);`,
      async (page, step) => {
        await step('initial-zero', async () => assert.equal(await page.locator('#value').textContent(), '0'));
        await step('mutate', () => page.evaluate(() => globalThis.__experiment.update()));
        await page.evaluate(() => { const h = globalThis.__experiment; h.values.renderedSize = document.getElementById('value').textContent; h.values.actualSize = h.actualSize(); });
        await step('assert-behavior', async () => assert.equal(await page.locator('#value').textContent(), snapshot ? '0' : '1'));
        await step('dispose', () => page.evaluate(() => { globalThis.__experiment.dispose(); globalThis.__experiment.disposals++; }));
      }, { packages: [row.package], provenance: { syntheticBehavioralChallenge: true, intendedBehavior: 'rendered size follows inserted entries', snapshot, diagnosticSilenceIsNotSuccess: true } });
  }
  for (const entry of extraCases) {
    const install = entry.app ? join(apps, entry.app) : retained.results.find(row => row.package === entry.package)?.retainedArtifacts.projectDir;
    assert(install, `Missing retained installation: ${entry.package ?? entry.app}`);
    await execute(entry.id, install, entry.source, entry.flow, { appCopy: entry.appCopy ?? false, packages: entry.packages ?? [entry.package], provenance: entry.provenance, patchCopy: entry.patchCopy });
  }
} finally { await browser.close(); report.finishedAt = new Date().toISOString(); save(); }
console.log(JSON.stringify({ results: report.results.length, failures: report.results.filter(row => row.harnessFailure).map(row => ({ id: row.id, message: row.harnessFailure.message })), feedback: report.results.map(row => ({ id: row.id, typingErrors: row.publishedTypingErrors.length, pageErrors: row.pageErrors.length, codes: row.feedback.map(item => item.code), values: row.values })) }, null, 2));
