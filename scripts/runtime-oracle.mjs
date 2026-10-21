#!/usr/bin/env node
// Runtime oracle: run small Solid 2 projects in Chrome under the dev build and
// compare the runtime's own diagnostics with the checker's verdict at the same
// site. It measures unproven clean claims directly: a dev diagnostic at a site
// where the checker reports nothing is a missing finding, which no "0 added"
// corpus comparison can see.
//
//   node scripts/runtime-oracle.mjs --cases fixtures/runtime-oracle --out <fresh.json>
//     [--only id,...] [--install <app with solid-js 2.x node_modules>]
//     [--browser <chromium>] [--concurrency 2]
//   env: SOLID_CHECKER_NATIVE_BIN, SOLID_TYPEFACTS_BIN
//
// A case is a directory holding an `App.tsx` and its other source files. Every
// exported function whose name starts with a capital is rendered as a
// component in its own root. The entry then drives the page itself: two passes
// dispatch a fixed set of DOM events on every element (the second reaches
// elements the first one revealed), so a case needs no per-site steps. An
// optional `oracle.json` adds a note, further feedback scenario steps, an
// `install` corpus app name (relative to the parent of --install), and the
// case's known open sites (each an unproven clean claim with its reason):
//   { "note": "what the case pins", "steps": [ ... ],
//     "open": [{ "code": "NO_OWNER_CLEANUP", "site": "App.tsx:13", "reason": "..." }] }
// The run exits 1 on an unlisted unproven-clean site, a listed site that no
// longer fires, a TypeScript error, or a harness error.
// A case may also observe the page for a static finding that no dev diagnostic
// covers (`behaviour`, fixtures/runtime-oracle/README.md). Each observation names
// a checker rule and the site it substantiates, and declares the value that
// shows the defect and the value that shows correct behaviour. Its verdict:
// behaviour-confirmed, behaviour-refuted (a false-positive candidate), or
// unproven-clean when the page shows the defect and no finding covers the site.
// A `reflected` observation instead scripts writes to one element. Each write names
// the text it sets (`value`) and a `probe` node bound to the same source; the page
// settles before and after it, and classifies it lost, reflected or unexpected. A
// malformed `behaviour` spec refuses its case: that case is a harness-error with the
// reason, and the other cases still run.
// Files named *.d.ts and node_modules directories are not copied: the case
// runs against the real packages of `--install`, never against a stub.
//
// Each case runs once through `solid-checker feedback run`, which records the
// dev diagnostics the runtime attributed to an authored site and the native
// findings for the same files. Per runtime diagnostic in a case file:
// - `covered-violation` / `covered-uncertifiable`: a native finding of that
//   kind, of a rule claiming the code, spans the diagnostic's line or the
//   line of an application frame on its stack;
// - `unproven-clean`: no native finding of a rule claiming that code there.
//   This is the defect class. A case TypeScript rejects against the real
//   typings is `type-error` and proves nothing: fix the case.
// Native violations with no diagnostic on their line are listed as
// `static-only`: either a false positive or a site the steps did not reach.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const options = {
  cases: join(repo, "fixtures/runtime-oracle"),
  install: resolve(repo, "../solid-checker-rc13-corpus/apps/helge-dev"),
  browser: null,
  tooling: null,
  concurrency: "2",
  only: "",
  out: null,
};
const argv = process.argv.slice(2);
for (let index = 0; index < argv.length; index += 2) {
  const key = argv[index].replace(/^--/, "");
  assert(key in options && argv[index + 1] !== undefined, `unknown or incomplete option ${argv[index]}`);
  options[key] = argv[index + 1];
}
assert(options.out, "--out <fresh.json> is required");
const output = resolve(options.out);
assert(!existsSync(output), "use a fresh output path");
assert(process.env.SOLID_CHECKER_NATIVE_BIN && process.env.SOLID_TYPEFACTS_BIN, "set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN");

const install = resolve(options.install);
const appsRoot = dirname(install);
const tooling = resolve(options.tooling ?? install);
const browser = options.browser ? resolve(options.browser) : defaultChromium();
const solidVersion = JSON.parse(readFileSync(join(install, "node_modules/solid-js/package.json"), "utf8")).version;
assert(solidVersion.startsWith("2."), `--install resolves solid-js ${solidVersion}, not 2.x`);
// Cases run inside the install so its node_modules is their ancestor.

function defaultChromium() {
  const cache = join(process.env.HOME, "Library/Caches/ms-playwright");
  const builds = readdirSync(cache).filter((name) => /^chromium-\d+$/.test(name)).sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]));
  assert(builds.length, "no playwright chromium; pass --browser");
  return join(cache, builds[0], "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing");
}

const compilerOptions = {
  target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve",
  jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM", "DOM.Iterable"], types: [],
  skipLibCheck: true, strict: true, noEmit: true,
};
const mainSource = (observations) => `${observations.length ? 'import { flush } from "solid-js";\n' : ""}import { render } from "@solidjs/web";
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

${behaviourProbe(observations)}// Drive every handler: bubbling events on every element, two passes apart.
const events: Array<[string, (type: string) => Event]> = [
  ...["click", "dblclick", "mousedown", "mouseup", "mouseover", "mouseout", "pointerdown", "pointerup", "contextmenu"]
    .map((type): [string, (type: string) => Event] => [type, (t) => new MouseEvent(t, { bubbles: true, cancelable: true })]),
  ...["keydown", "keyup"].map((type): [string, (type: string) => Event] => [type, (t) => new KeyboardEvent(t, { bubbles: true, key: "a" })]),
  ...["focus", "blur", "focusin", "focusout"].map((type): [string, (type: string) => Event] => [type, (t) => new FocusEvent(t, { bubbles: true })]),
  ...["input", "change", "submit", "scroll", "wheel"].map((type): [string, (type: string) => Event] => [type, (t) => new Event(t, { bubbles: true, cancelable: true })]),
];
const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));
async function exercise() {
${observations.length ? "  await behave();\n" : ""}  await pause(200);
  for (let pass = 0; pass < 2; pass += 1) {
    for (const element of Array.from(root.querySelectorAll("*"))) {
      for (const [type, make] of events) {
        try {
          element.dispatchEvent(make(type));
        } catch (error) {
          console.error("handler threw", type, error);
        }
      }
    }
    await pause(200);
  }
  // Give effects, timers and settled callbacks a moment, then mark readiness.
  await pause(400);
  document.getElementById("ready")!.textContent = "settled";
}
void exercise();
`;
const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div><p id="ready">running</p><script type="module" src="/main.tsx"></script></body></html>\n`;
// The checker rules that claim each runtime diagnostic code. A code no rule
// claims (REACTIVITY_HALTED, an uncaught error's consequence) is reported as
// `no-rule`, never as a missing finding.
const codeRules = {
  STRICT_READ_UNTRACKED: ["strict-read-untracked", "no-destructure", "reactive-handler-frozen", "components-return-once"],
  NO_OWNER_CLEANUP: ["missing-owner"],
  // An apply-phase createEffect warns this code, as the primitive-in-directive-application page states.
  NO_OWNER_EFFECT: ["missing-owner", "primitive-in-directive-application"],
  NO_OWNER_BOUNDARY: ["missing-owner"],
  // Thrown, not reported: `useContext` with no owner (ADR 0352). See `thrownCodes`.
  NO_OWNER_CONTEXT: ["missing-owner"],
  // ADR 0352: a default-less context with no provider throws when its reader renders.
  CONTEXT_NOT_FOUND: ["missing-context-provider"],
  SETTLED_CLEANUP_UNOWNED: ["missing-owner"],
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["reactive-write-in-owned-scope"],
  CLEANUP_IN_FORBIDDEN_SCOPE: ["leaf-owner-forbidden-call"],
  PRIMITIVE_IN_FORBIDDEN_SCOPE: ["leaf-owner-forbidden-call"],
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["async-outside-loading-boundary"],
  PENDING_ASYNC_UNTRACKED_READ: ["pending-async-unsuspendable-read"],
  SYNC_NODE_RECEIVED_ASYNC: ["sync-computation-received-async"],
  UNTRACKED_READ_AFTER_AWAIT: ["reactive-read-after-await"],
  MISSING_EFFECT_FN: ["missing-effect-function"],
  ACTION_CALLED_IN_OWNED_SCOPE: ["action-called-in-owned-scope"],
  // Thrown, not reported: see `thrownCodes`.
  RESOLVE_IN_REACTIVE_SCOPE: ["resolve-in-tracked-scope"],
  UNTIL_IN_REACTIVE_SCOPE: ["until-in-tracked-scope"],
  FLUSH_IN_ACTION: ["flush-in-action"],
  FLUSH_IN_EFFECT_CALLBACK: ["flush-in-effect-callback"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["leaf-owner-forbidden-call"],
  DYNAMIC_STATIC_PROMISE: ["static-dynamic-async-source"],
};
// Dev-build errors that are thrown with no diagnostic code (rc.13
// `@solidjs/signals/dist/dev.js`, `dev-shared.js`; `@solidjs/web/dist/web.dev.js`),
// named here by their message so the uncaught error, which the feedback run
// maps to its throwing site (`runtime-exception` observations), is classified
// like a coded diagnostic. A thrown `[CODE] ...` that is also reported as a
// diagnostic (SYNC_NODE_RECEIVED_ASYNC) is not listed: it would count twice.
const thrownCodes = [
  [/^Cannot call resolve inside a reactive scope/, "RESOLVE_IN_REACTIVE_SCOPE"],
  [/^Cannot call until inside a reactive scope/, "UNTIL_IN_REACTIVE_SCOPE"],
  [/^\[FLUSH_IN_ACTION\]/, "FLUSH_IN_ACTION"],
  [/^Cannot call flush\(\) from inside onSettled or createTrackedEffect/, "FLUSH_IN_TRACKED_QUEUE_CALLBACK"],
  [/^dynamic\(\): a static source must resolve synchronously/, "DYNAMIC_STATIC_PROMISE"],
  // `getContext` checks the owner first (rc.13 `dev-shared.js`), whatever the default.
  [/^Context can only be accessed under a reactive root/, "NO_OWNER_CONTEXT"],
  [/^Context must either be created with a default value/, "CONTEXT_NOT_FOUND"],
];
// Withholdings that cover any rule at their site.
const withholdingRules = ["reactive-dispatch-unresolved"];
const settle = { action: "wait-for-text", selector: "#ready", text: "settled" };
// Reads the page's behavioural report back: its checkpoint text is the JSON list runCase parses.
const probeStep = { action: "wait-for-selector", selector: "#oracle-behaviour" };

function sourceFiles(directory, base = directory) {
  const files = [];
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    if (name === "node_modules" || name.startsWith(".")) continue;
    if (statSync(path).isDirectory()) files.push(...sourceFiles(path, base));
    else if (/\.(tsx?|jsx?)$/.test(name) && !name.endsWith(".d.ts")) files.push(relative(base, path));
  }
  return files;
}

function run(command, args, cwd) {
  return new Promise((done) => {
    const child = spawn(command, args, { cwd, env: process.env, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), 180000);
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("close", (status) => { clearTimeout(timer); done({ status, stdout, stderr }); });
  });
}

const behaviourKinds = ["text", "window", "identity", "thrown", "reflected"];
// A write's expectation: lost is the defect, reflected the correct behaviour (the two values a reflected observation has).
const writeExpectations = ["lost", "reflected"];
// The page reports these names for an identity or thrown observation; any other value is a case error.
const behaviourValues = { identity: ["same", "replaced", "missing"], thrown: ["threw", "none"] };
const isPattern = (expected) => typeof expected === "object" && expected !== null && Object.keys(expected).join() === "pattern"
  && typeof expected.pattern === "string";
const matches = (value, expected) => isPattern(expected) ? typeof value === "string" && new RegExp(expected.pattern).test(value)
  : JSON.stringify(value) === JSON.stringify(expected);
// Why a reflected observation's writes are unexpected: a no-op write, text that changed without the write's value
// (or already held it), or an outcome other than the write's expect. Empty when every write is as predicted.
const writeReasons = (writes) => (Array.isArray(writes) ? writes : []).flatMap((write) => write.noop
  ? [`no-op write ${write.click}: probe ${write.probe} did not change`]
  : write.observed === "unexpected" ? [`write ${write.click}: ${write.reason}`]
  : write.match ? [] : [`write ${write.click}: expected ${write.expect}, observed ${write.observed}`]);
// A reflected observation's page: the defect when any write leaves its text unchanged (lost), the correct
// behaviour when every write changes it (reflected). No writes, a reason, or any other result is unexpected.
const reflectedOutcome = (writes) => !Array.isArray(writes) || writes.length === 0 || writeReasons(writes).length
  ? "unexpected" : writes.some((write) => write.observed === "lost") ? "defect" : "correct";

// The rule ids a behaviour may name: each code's rules, and each rule page (a rule with neither is not one the checker reports).
const knownRules = new Set([
  ...Object.values(codeRules).flat(),
  ...readdirSync(join(repo, "docs/rules")).filter((name) => name.endsWith(".md") && name !== "README.md").map((name) => name.slice(0, -".md".length)),
]);
const compiles = (pattern) => {
  try { new RegExp(pattern); return true; } catch { return false; }
};

// A case's `behaviour` list, validated: a malformed observation is a case error, never a silent skip.
// Every refusal throws, and the worker records it as that case's harness-error.
function behaviourSpec(id, spec, source, files) {
  assert(spec.behaviour === undefined || Array.isArray(spec.behaviour), `${id}: behaviour must be a list`);
  assert(spec.behaviour === undefined || spec.behaviour.length > 0, `${id}: behaviour is empty; omit the key`);
  return (spec.behaviour ?? []).map((entry, index) => {
    const where = `${id}: behaviour[${index}]`;
    assert(entry !== null && typeof entry === "object" && !Array.isArray(entry), `${where} is an object`);
    assert(typeof entry.rule === "string" && knownRules.has(entry.rule), `${where}: rule ${JSON.stringify(entry.rule)} is not a checker rule (a codeRules value or docs/rules/<rule>.md)`);
    const site = /^(.+):([1-9]\d*)$/.exec(entry.site ?? "");
    assert(site && files.includes(site[1]), `${where} needs a site App.tsx:<line> inside the case`);
    const line = Number(site[2]);
    assert(line <= readFileSync(join(source, site[1]), "utf8").split("\n").length, `${where} site is past the end of ${site[1]}`);
    const [kind, ...extra] = Object.keys(entry.observe ?? {});
    const reflected = kind === "reflected";
    // A reflected observation's writes are its steps: it takes no act, and names no defect or correct (lost and reflected are fixed).
    assert(entry.writes === undefined || reflected, `${where} writes is for observe.reflected`);
    assert(!reflected || (entry.act === undefined && entry.defect === undefined && entry.correct === undefined), `${where} reflected takes writes, not act, defect or correct`);
    const act = entry.act ?? [];
    assert(Array.isArray(act) && act.every((step) => typeof step?.click === "string" && step.click.trim()), `${where} act steps are { "click": selector }`);
    assert(extra.length === 0 && behaviourKinds.includes(kind), `${where} observe names exactly one of ${behaviourKinds.join(", ")}`);
    if (reflected) {
      assert(typeof entry.observe.reflected === "string" && entry.observe.reflected.trim(), `${where} observe.reflected is a selector`);
      assert(Array.isArray(entry.writes) && entry.writes.length > 0, `${where} reflected needs a non-empty writes list`);
      const writes = entry.writes.map((write, position) => {
        const at = `${where} writes[${position}]`;
        assert(write !== null && typeof write === "object" && !Array.isArray(write), `${at} is an object`);
        assert(Object.keys(write).sort().join() === "click,expect,probe,value",
          `${at} is { "click": selector, "expect": "lost" or "reflected", "value": text, "probe": selector }`);
        assert(typeof write.click === "string" && write.click.trim(), `${at} click is a selector`);
        assert(writeExpectations.includes(write.expect), `${at} expect is ${writeExpectations.join(" or ")}`);
        assert(typeof write.value === "string" && write.value.trim(), `${at} value is the non-empty text the write sets`);
        assert(typeof write.probe === "string" && write.probe.trim(), `${at} probe is a selector`);
        assert(write.probe.trim() !== entry.observe.reflected.trim(), `${at} probe is the observed selector; a probe is a separate node`);
        return { click: write.click, expect: write.expect, value: write.value, probe: write.probe };
      });
      // A click listed twice is one write with two expectations, or one claim twice.
      const clicks = writes.map((write) => write.click);
      assert(new Set(clicks).size === clicks.length, `${where} lists a click twice; each write is listed once`);
      return { rule: entry.rule, site: entry.site, file: site[1], line, kind, act, observe: entry.observe, writes, defect: "lost", correct: "reflected" };
    }
    if (kind === "thrown") assert(entry.observe.thrown === true, `${where} observe.thrown is true`);
    else assert(typeof entry.observe[kind] === "string" && entry.observe[kind].trim(), `${where} observe.${kind} is a path or a selector`);
    for (const expected of ["defect", "correct"]) {
      assert(expected in entry, `${where} needs ${expected}`);
      if (isPattern(entry[expected])) {
        assert(kind === "text" || kind === "window", `${where} ${expected}: a pattern matches text or a window value`);
        assert(entry[expected].pattern !== "", `${where} ${expected}: an empty pattern matches every page`);
        assert(compiles(entry[expected].pattern), `${where} ${expected}: ${JSON.stringify(entry[expected].pattern)} is not a regular expression`);
      } else if (behaviourValues[kind]) {
        assert(behaviourValues[kind].includes(entry[expected]), `${where} ${expected} is one of ${behaviourValues[kind].join(", ")}`);
      }
    }
    const { defect, correct } = entry;
    // The two sides must differ as text: identical literals, or identical patterns, would be one claim twice.
    assert(JSON.stringify(defect) !== JSON.stringify(correct), `${where}: defect and correct must differ`);
    // Two patterns are told apart by examples (below); a literal side must not be a page the other pattern accepts.
    if (isPattern(defect) && isPattern(correct)) {
      assert("defectExample" in entry && "correctExample" in entry, `${where}: two patterns need defectExample and correctExample`);
    } else if (isPattern(defect)) {
      assert(!matches(correct, defect), `${where}: correct ${JSON.stringify(correct)} matches the defect pattern`);
    } else if (isPattern(correct)) {
      assert(!matches(defect, correct), `${where}: defect ${JSON.stringify(defect)} matches the correct pattern`);
    }
    // An example is a page text: it must match its own side, and not the other side.
    for (const [name, side, other] of [["defectExample", defect, correct], ["correctExample", correct, defect]]) {
      if (!(name in entry)) continue;
      assert(typeof entry[name] === "string", `${where} ${name} is a string`);
      assert(matches(entry[name], side), `${where} ${name} ${JSON.stringify(entry[name])} does not match its own side`);
      assert(!matches(entry[name], other), `${where} ${name} ${JSON.stringify(entry[name])} matches the other side too`);
    }
    return { rule: entry.rule, site: entry.site, file: site[1], line, kind, act, observe: entry.observe, defect, correct };
  });
}

// The probe code for a case's observations. It runs before the exercise, because
// the exercise's events would change the state the scripted clicks start from.
function behaviourProbe(observations) {
  if (!observations.length) return "";
  const payload = JSON.stringify(observations.map(({ act, observe, writes }) => ({ act, observe, writes })));
  return `// Behavioural observations: each runs its scripted clicks (a flush after each), then
// reads the page. The results go to a <pre> that the feedback run reads back.
const observations: any[] = ${payload};
// A write-reflected observation. Each write is bracketed by settle(): the observed text and the write's
// probe are read once the page has drained its microtasks and timers, and must hold across two timer ticks
// (settleLimitMs at most), so a deferred update is part of the write it follows. The click then runs
// (click, then a flush) under writeLimitMs. A missing element, a step that throws, or a step still running
// after the limit fails the observation. flush() is synchronous in rc.13, so the limit binds only an
// asynchronous flush; a page that never returns from a step ends the feedback run (harness-error).
// The probe is bound to the write's source: when it does not change, the write was a no-op. The write is
// lost when the observed text is unchanged, reflected when the text changed to contain the write's value
// (and did not contain it before), and unexpected otherwise.
const writeLimitMs = 2000;
const settleLimitMs = 500;
const normal = (text: string): string => text.trim().replace(/\\s+/g, " ");
function bounded<T>(step: () => T): Promise<T> {
  return new Promise<T>((done, fail) => {
    const timer = setTimeout(() => fail(new Error("timeout after " + writeLimitMs + " ms")), writeLimitMs);
    Promise.resolve().then(step).then(
      (value) => { clearTimeout(timer); done(value); },
      (error) => { clearTimeout(timer); fail(error); });
  });
}
function textAt(selector: string): string {
  const element = document.querySelector(selector);
  if (element === null) throw new Error("no element matches " + selector);
  return normal(element.textContent ?? "");
}
const timerTick = () => new Promise<void>((done) => setTimeout(done, 0));
// The selectors' texts, read once they hold across two consecutive timer ticks. Microtasks queued before a
// tick run before it, so an update that lands in that window is part of the text this returns.
async function settle(selectors: string[]): Promise<string[]> {
  const deadline = Date.now() + settleLimitMs;
  let previous = selectors.map((selector) => textAt(selector));
  for (;;) {
    await timerTick();
    const middle = selectors.map((selector) => textAt(selector));
    await timerTick();
    const current = selectors.map((selector) => textAt(selector));
    if (JSON.stringify(previous) === JSON.stringify(middle) && JSON.stringify(middle) === JSON.stringify(current)) return current;
    if (Date.now() >= deadline) throw new Error("text still changing after " + settleLimitMs + " ms: " + selectors.join(", "));
    previous = current;
  }
}
async function reflectWrites(selector: string, writes: any[]): Promise<unknown[]> {
  const rows: unknown[] = [];
  for (const write of writes) {
    const probeNode = document.querySelector(write.probe);
    if (probeNode !== null && probeNode === document.querySelector(selector)) throw new Error("probe " + write.probe + " is the observed node");
    const [before, probeBefore] = await settle([selector, write.probe]);
    const errors: string[] = [];
    const onError = (event: ErrorEvent) => { errors.push(String(event.message)); };
    addEventListener("error", onError);
    try {
      const target = document.querySelector(write.click);
      if (!(target instanceof HTMLElement)) throw new Error("no HTMLElement matches " + write.click);
      await bounded(() => { target.click(); flush(); });
    } finally {
      removeEventListener("error", onError);
    }
    if (errors.length) throw new Error("write " + write.click + " threw: " + errors[0]);
    const [after, probeAfter] = await settle([selector, write.probe]);
    const value = normal(write.value);
    const reason = after === before ? null
      : !after.includes(value) ? "text became " + JSON.stringify(after) + " without the value " + JSON.stringify(value)
      : before.includes(value) ? "the value " + JSON.stringify(value) + " was already in the text before the write"
      : null;
    const observed = after === before ? "lost" : reason === null ? "reflected" : "unexpected";
    rows.push({
      click: write.click, expect: write.expect, value, probe: write.probe, before, after, probeBefore, probeAfter,
      noop: probeAfter === probeBefore, observed, reason, match: observed === write.expect,
    });
  }
  return rows;
}
function readObserved(spec: any, reference: Element | null, errors: string[]): unknown {
  if (spec.text !== undefined) return document.querySelector(spec.text)?.textContent ?? null;
  if (spec.window !== undefined) return spec.window.split(".").reduce((value: any, key: string) => value?.[key], globalThis) ?? null;
  if (spec.identity !== undefined) {
    if (reference === null) return null;
    const now = document.querySelector(spec.identity);
    return now === null ? "missing" : now === reference ? "same" : "replaced";
  }
  return errors.length ? "threw" : "none";
}
async function behave(): Promise<void> {
  await flush();
  const results: unknown[] = [];
  for (const [index, { act, observe: spec, writes }] of observations.entries()) {
    try {
      if (spec.reflected !== undefined) {
        results.push({ index, writes: await reflectWrites(spec.reflected, writes) });
        continue;
      }
      const reference = spec.identity === undefined ? null : document.querySelector(spec.identity);
      const errors: string[] = [];
      const onError = (event: ErrorEvent) => { errors.push(String(event.message)); };
      addEventListener("error", onError);
      try {
        for (const step of act) {
          const element = document.querySelector(step.click);
          if (!(element instanceof HTMLElement)) throw new Error("no HTMLElement matches " + step.click);
          element.click();
          await flush();
        }
      } finally {
        removeEventListener("error", onError);
      }
      results.push({ index, observed: readObserved(spec, reference, errors), detail: errors[0] ?? null });
    } catch (error) {
      results.push({ index, error: error instanceof Error ? error.message : String(error) });
    }
  }
  const output = document.createElement("pre");
  output.id = "oracle-behaviour";
  output.textContent = JSON.stringify(results);
  document.body.append(output);
}
`;
}

function parseProbes(text) {
  try {
    const rows = JSON.parse(text ?? "null");
    return Array.isArray(rows) ? rows : [];
  } catch {
    return [];
  }
}

async function runCase(id) {
  const source = join(resolve(options.cases), id);
  const specPath = join(source, "oracle.json");
  const spec = existsSync(specPath) ? JSON.parse(readFileSync(specPath, "utf8")) : {};
  const caseInstall = spec.install ? resolve(appsRoot, spec.install) : install;
  const caseSolid = JSON.parse(readFileSync(join(caseInstall, "node_modules/solid-js/package.json"), "utf8")).version;
  assert(caseSolid === solidVersion, `${id}: install has solid-js ${caseSolid}, expected ${solidVersion}`);
  const dir = join(caseInstall, ".solid-checker-runtime-oracle", id);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const files = sourceFiles(source);
  assert(files.includes("App.tsx"), `${id}: no App.tsx`);
  const observations = behaviourSpec(id, spec, source, files);
  for (const file of files) cpSync(join(source, file), join(dir, file));
  writeFileSync(join(dir, "main.tsx"), mainSource(observations));
  writeFileSync(join(dir, "index.html"), html);
  writeFileSync(join(dir, "scenario.json"), JSON.stringify({ schemaVersion: 1, steps: [settle, ...(observations.length ? [probeStep] : []), ...(spec.steps ?? [])] }));
  writeFileSync(join(dir, "tsconfig.json"), JSON.stringify({ compilerOptions, files: ["main.tsx", ...files] }, null, 2));
  const started = Date.now();
  const result = await run(process.execPath, [join(repo, "packages/cli/bin/solid-checker.mjs"), "feedback", "run",
    "--project", join(dir, "tsconfig.json"), "--scenario", join(dir, "scenario.json"),
    "--browser", browser, "--tooling", tooling, "--native-bin", process.env.SOLID_CHECKER_NATIVE_BIN], dir);
  const wallMs = Date.now() - started;
  let report = null;
  try { report = JSON.parse(result.stdout.slice(result.stdout.indexOf("{"))); } catch { /* recorded below */ }
  if (report) writeFileSync(join(dir, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
  if (!report) return { id, verdict: "harness-error", error: (result.stderr || result.stdout).slice(-1500), wallMs };
  if (!report.analysis || !report.execution) return {
    id, verdict: report.typingErrorCount ? "type-error" : "harness-error",
    error: report.failure?.message ?? "feedback report has no analysis or execution", wallMs,
  };

  const caseFile = (path) => path && path.startsWith(dir + "/") && !path.endsWith("/main.tsx") ? relative(dir, path) : null;
  const findings = (report.analysis?.findings ?? [])
    .filter((finding) => caseFile(finding.primaryLocation?.path))
    .map((finding) => ({
      file: caseFile(finding.primaryLocation.path), rule: finding.rule, id: finding.id, kind: finding.kind,
      line: finding.primaryLocation.line, endLine: finding.primaryLocation.endLine ?? finding.primaryLocation.line,
    }));
  const seen = new Set();
  // An uncaught error a coded rule claims, as a diagnostic row at its mapped
  // throwing site (the observation carries a byte offset, no line).
  const thrown = (report.observations ?? []).flatMap((observation) => {
    const code = observation.kind === "runtime-exception"
      && thrownCodes.find(([pattern]) => pattern.test(observation.message ?? ""))?.[1];
    const path = observation.location?.path;
    if (!code || !caseFile(path)) return [];
    const before = readFileSync(path).subarray(0, observation.location.startByte).toString();
    const line = before.split("\n").length;
    const column = before.length - before.lastIndexOf("\n");
    return [{ code, message: observation.message, site: { location: { path }, line, column }, stack: [] }];
  });
  // Each diagnostic's positions (its site, then its application frames) and the rules its code claims.
  const framed = [];
  const diagnostics = [...(report.execution?.diagnostics ?? []), ...thrown]
    .filter((row) => caseFile(row.site?.location?.path))
    // The two event passes repeat a diagnostic; one row per code and site.
    .filter((row) => {
      const key = `${row.code} ${row.site.location.path}:${row.site.line}:${row.site.column}`;
      return !seen.has(key) && seen.add(key);
    })
    .map((row) => {
      const file = caseFile(row.site.location.path);
      const line = row.site.line;
      const rules = codeRules[row.code];
      // The read site first, then each application frame below it: a finding
      // may anchor at the call that led to the read.
      const positions = [row.site, ...(row.stack ?? [])]
        .map((frame) => ({ file: caseFile(frame.location.path), line: frame.line }))
        .filter((position) => position.file);
      const covering = findings.filter((finding) => positions.some((position) => finding.file === position.file
        && finding.line <= position.line && position.line <= finding.endLine)
        && (rules?.includes(finding.rule) || withholdingRules.includes(finding.rule)));
      framed.push({ rules: rules ?? [], positions });
      return {
        code: row.code, file, line, column: row.site.column, operation: row.operation?.in ?? null,
        siteKind: row.siteKind ?? null, message: row.message?.slice(0, 160),
        verdict: !rules ? "no-rule"
          : covering.some((finding) => finding.kind === "violation") ? "covered-violation"
          : covering.length ? "covered-uncertifiable" : "unproven-clean",
        stack: positions.slice(1).map((position) => `${position.file}:${position.line}`),
        covering: covering.map((finding) => `${finding.file}:${finding.line} ${finding.id} ${finding.kind} ${finding.rule}`),
      };
    });
  // A violation no diagnostic covers: not at a diagnostic's line, nor at a frame of a diagnostic whose code claims its rule
  // (the runtime attributes a read to the call that led to it, and the read's line is a frame).
  const uncovered = findings.filter((finding) => finding.kind === "violation"
    && !["prefer-show", "prefer-for"].includes(finding.rule)
    && !diagnostics.some((row) => row.file === finding.file && finding.line <= row.line && row.line <= finding.endLine)
    && !framed.some(({ rules, positions }) => rules.includes(finding.rule)
      && positions.some((position) => position.file === finding.file && finding.line <= position.line && position.line <= finding.endLine)));
  const failure = report.failure?.message ?? report.execution?.failure?.message ?? null;
  const typingErrors = report.typingErrorCount ?? null;
  // A known open site is listed in oracle.json with its reason; it is not a
  // pass, it is debt the gate keeps visible. A listed site that no longer
  // fires is stale and fails too, so a fix updates the list. A behavioural
  // observation is listed by its `rule` and `site` instead of a `code`.
  const open = new Map((spec.open ?? []).filter((entry) => entry.code).map((entry) => [`${entry.code} ${entry.site}`, entry.reason]));
  const openBehaviour = new Map((spec.open ?? []).filter((entry) => entry.rule).map((entry) => [`behaviour ${entry.rule} ${entry.site}`, entry.reason]));
  for (const row of diagnostics) {
    if (row.verdict === "unproven-clean" && open.has(`${row.code} ${row.file}:${row.line}`)) {
      row.verdict = "known-open";
      row.reason = open.get(`${row.code} ${row.file}:${row.line}`);
    }
  }
  const stale = [...open.keys()].filter((key) => !diagnostics.some((row) => row.verdict === "known-open" && `${row.code} ${row.file}:${row.line}` === key));
  // The page's report of each observation: the probe checkpoint, a JSON list keyed by observation index.
  const probes = parseProbes((report.execution?.checkpoints ?? []).find((row) => row.selector === "#oracle-behaviour")?.text);
  const behaviour = observations.map((observation, index) => {
    const probe = probes.find((row) => row.index === index) ?? null;
    const key = `behaviour ${observation.rule} ${observation.site}`;
    const covering = findings.filter((finding) => finding.file === observation.file && finding.rule === observation.rule
      && finding.line <= observation.line && observation.line <= finding.endLine);
    const reflected = observation.kind === "reflected";
    const outcome = !probe || probe.error !== undefined ? "unexpected"
      : reflected ? reflectedOutcome(probe.writes)
      : probe.observed === undefined ? "unexpected"
      : matches(probe.observed, observation.defect) ? "defect"
      : matches(probe.observed, observation.correct) ? "correct" : "unexpected";
    const violated = covering.some((finding) => finding.kind === "violation");
    const verdict = outcome === "unexpected" ? "behaviour-unexpected"
      : outcome === "defect" ? (violated ? "behaviour-confirmed" : covering.length ? "covered-uncertifiable" : openBehaviour.has(key) ? "known-open" : "unproven-clean")
      : violated ? "behaviour-refuted" : "behaviour-clean";
    // A reflected observation's reasons (a no-op write, an unexplained change, an outcome other than expect) are its error.
    const reasons = reflected && probe ? (Array.isArray(probe.writes) ? writeReasons(probe.writes) : ["no write results"]) : [];
    return {
      source: "behaviour", rule: observation.rule, site: observation.site, file: observation.file, line: observation.line,
      observation: observation.kind, outcome, verdict,
      observed: reflected ? probe?.writes?.map((write) => write.observed) ?? null : probe?.observed ?? null,
      ...(reflected ? { writes: probe?.writes ?? null } : {}),
      expected: { defect: observation.defect, correct: observation.correct },
      detail: probe?.detail ?? null, error: probe ? (probe.error ?? (reasons.join("; ") || null)) : "no probe result",
      covering: covering.map((finding) => `${finding.file}:${finding.line} ${finding.id} ${finding.kind} ${finding.rule}`),
      ...(verdict === "known-open" ? { reason: openBehaviour.get(key) } : {}),
    };
  });
  // A violation with no dev code is classified by the behaviour observed for its rule at its lines.
  const staticOnly = uncovered.map((finding) => {
    const evidence = behaviour.filter((row) => row.rule === finding.rule && row.file === finding.file
      && finding.line <= row.line && row.line <= finding.endLine);
    const defect = evidence.some((row) => row.outcome === "defect");
    const correct = evidence.some((row) => row.outcome === "correct");
    const classification = defect && correct ? "behaviour-conflict" : defect ? "behaviour-confirmed" : correct ? "behaviour-refuted" : "static-only";
    return { ...finding, classification };
  });
  const staleBehaviour = [...openBehaviour.keys()].filter((key) => !behaviour.some((row) => row.verdict === "known-open" && `behaviour ${row.rule} ${row.site}` === key));
  const rows = [...diagnostics, ...behaviour];
  const verdict = typingErrors ? "type-error"
    : rows.some((row) => row.verdict === "unproven-clean") ? "unproven-clean"
    : stale.length || staleBehaviour.length ? "stale-open"
    : behaviour.some((row) => row.verdict === "behaviour-unexpected") || staticOnly.some((finding) => finding.classification === "behaviour-conflict") ? "harness-error"
    : rows.some((row) => row.verdict === "known-open") ? "known-open"
    : failure ? "harness-error"
    : rows.some((row) => row.verdict === "covered-uncertifiable") ? "covered-uncertifiable"
    // A refuted finding is a false-positive candidate: it is reported ahead of a covered verdict, never hidden under one.
    : behaviour.some((row) => row.verdict === "behaviour-refuted") ? "behaviour-refuted"
    : diagnostics.length ? "covered-violation"
    : staticOnly.some((finding) => finding.classification === "static-only") ? "static-only"
    : behaviour.some((row) => row.verdict === "behaviour-confirmed") ? "behaviour-confirmed"
    : behaviour.length ? "behaviour-clean" : "quiet";
  return {
    id, verdict, note: spec.note ?? null, wallMs, typingErrors, failure, stale: [...stale, ...staleBehaviour], diagnostics, behaviour, staticOnly, findings,
    unattributed: (report.execution?.diagnostics ?? []).filter((row) => !caseFile(row.site?.location?.path)).map((row) => row.code),
    pageErrors: (report.execution?.pageErrors ?? []).map((error) => error.message.slice(0, 200)),
  };
}

const ids = readdirSync(resolve(options.cases))
  .filter((name) => existsSync(join(resolve(options.cases), name, "App.tsx")))
  .filter((name) => !options.only || options.only.split(",").includes(name))
  .sort();
const results = [];
let next = 0;
async function worker() {
  while (next < ids.length) {
    const id = ids[next++];
    // A case that cannot run (a refused spec, an unreadable case) fails that case alone, with its reason.
    const result = await runCase(id).catch((error) => ({
      id, verdict: "harness-error", error: error instanceof Error ? error.message : String(error), wallMs: 0,
    }));
    results.push(result);
    process.stderr.write(`${results.length}/${ids.length} ${result.id}: ${result.verdict}\n`);
  }
}
await Promise.all(Array.from({ length: Number(options.concurrency) }, worker));
results.sort((left, right) => left.id.localeCompare(right.id));
const tally = results.reduce((counts, row) => ({ ...counts, [row.verdict]: (counts[row.verdict] ?? 0) + 1 }), {});
writeFileSync(output, `${JSON.stringify({ solid: solidVersion, install, browser, cases: results.length, tally, results }, null, 2)}\n`);
console.log(JSON.stringify(tally));
const failing = results.filter((row) => ["unproven-clean", "stale-open", "type-error", "harness-error"].includes(row.verdict));
for (const row of failing) {
  console.error(`${row.id}: ${row.verdict}`);
  for (const diagnostic of row.diagnostics ?? []) {
    if (diagnostic.verdict === "unproven-clean") console.error(`  ${diagnostic.code} ${diagnostic.file}:${diagnostic.line} (no finding along ${diagnostic.stack.join(", ") || "the site"})`);
  }
  for (const observation of row.behaviour ?? []) {
    if (["unproven-clean", "behaviour-unexpected"].includes(observation.verdict)) console.error(`  ${observation.rule} ${observation.site} (${observation.observation}: observed ${JSON.stringify(observation.observed)}; ${observation.verdict}${observation.error ? ": " + observation.error : ""})`);
  }
  for (const key of row.stale ?? []) console.error(`  stale open entry: ${key}`);
  if (row.error) console.error(`  ${row.error.split("\n").slice(-3).join("\n  ")}`);
}
if (failing.length) process.exit(1);
