// The SSR host (its own run, opt-in like witness.mjs): SC7005 http-response-after-flush judged by the response head the pinned rc.17
// server runtime sends. Each template declares a response header or status (`httpHeader("x-sc", …)`, `httpStatus(410)`) in one place;
// the program is compiled for SSR by the pinned `@solidjs/compiler` (`generate: "ssr"`, dev), rendered in Node by `renderToStream` in a
// request event, and its `Response` (from `createSSRResponse`, which commits the head at the shell flush) says whether the declaration
// reached the head (`applied`) or not (`dropped`: `httpStatus`/`httpHeader` are no-ops once `response.committed`). The checker runs on
// the same App.tsx as a streaming-SSR project (`--runtime-target node --rendering streaming-ssr`, beside a server entry that renders it).
//
// SC7005 is uncertifiable by design (request-time ordering decides the race; docs/rules/http-response-after-flush.md), so the judge
// never needs a violation: a dropped declaration is covered by any SC7005 finding at its line, and an applied one with an SC7005
// obligation is `uncertifiable` (the rule's over-approximation, counted, never a failure). An advisory or uncertifiable answer is never
// a violation here. A runtime answer that is not the template's design (`not-as-designed`) is the template's or the premise's problem
// and needs a look; a render that fails is `harness`.
//
// SC7006 (valid-use-server) and SC7007 (server-function-rich-argument) are not oracled here: see scripts/fuzz/README.md (SSR host).
//
//   node scripts/fuzz/ssr.mjs --out <fresh dir> [--checker-bin <solid-checker-rust>] [--typefacts <solid-typefacts>] [--tsc <tsrs>]
//     [--install <scripts/fuzz/install>] [--templates a,b]
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire, stripTypeScriptTypes } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkerEnv, checkerResult } from "./checker.mjs";
import { DEFAULT_FAST_INSTALL } from "./fast.mjs";
import { guardGridExit, partialRows } from "./partial.mjs";
import { resolveTsc, tsconfigFor, typecheck } from "./typecheck.mjs";
import { NODE } from "../lib/node-runtime.mjs";

const here = dirname(fileURLToPath(import.meta.url));

/** The two declarations, each read back from the head by `observe`. */
export const CALLS = {
  header: { imported: "httpHeader", call: (value) => `httpHeader("x-sc", ${value})`, observe: (head) => head.headers?.["x-sc"] !== undefined },
  status: { imported: "httpStatus", call: () => "httpStatus(410)", observe: (head) => head.status === 410 },
};

// A source that settles after the shell flush (a timer), and one that settles before it (a microtask).
const LATE = "const wait = (): Promise<number> => new Promise((done) => setTimeout(() => done(1), 30));";

/**
 * The templates: `design` is what the rc.17 runtime does with the declaration (measured 2026-11-03 on the pinned install, probes in
 * the README). `misuse` ones run the declaration after a pending read below a `<Loading>` boundary, so it runs only once the boundary
 * settles, after the head committed; `legal` ones run it at shell time. `$CALL` is the declaration, `$VALUE` a string the template
 * has at that point.
 */
export const TEMPLATES = [
  { id: "m-body-after-read", kind: "misuse", design: "dropped", text: `
${LATE}
function Inner() {
  const v = createMemo(async () => await wait());
  const n = v();
  $CALL;
  return <b>{n}</b>;
}
export default function App() {
  return <main><Loading fallback={<p>loading</p>}><Inner /></Loading></main>;
}` },
  { id: "m-props-after-read", kind: "misuse", design: "dropped", text: `
${LATE}
function Inner(props: { n: number }) {
  if (props.n === 1) $CALL;
  return <b>{props.n}</b>;
}
export default function App() {
  const v = createMemo(async () => await wait());
  return <main><Loading fallback={<p>loading</p>}><Inner n={v()} /></Loading></main>;
}` },
  { id: "m-accessor-prop", kind: "misuse", design: "dropped", text: `
${LATE}
function Inner(props: { v: () => number }) {
  const n = props.v();
  $CALL;
  return <b>{n}</b>;
}
export default function App() {
  const v = createMemo(async () => await wait());
  return <main><Loading fallback={<p>loading</p>}><Inner v={v} /></Loading></main>;
}` },
  { id: "m-jsx-hole", kind: "misuse", design: "dropped", text: `
${LATE}
export default function App() {
  const v = createMemo(async () => await wait());
  return <main><Loading fallback={<p>loading</p>}><b>{(String(v()), $CALL, "t")}</b></Loading></main>;
}` },
  { id: "m-nested", kind: "misuse", design: "dropped", text: `
${LATE}
function Deep(props: { n: number }) {
  if (props.n === 1) $CALL;
  return <i>{props.n}</i>;
}
function Inner() {
  const v = createMemo(async () => await wait());
  return <b><Deep n={v()} /></b>;
}
export default function App() {
  return <main><Loading fallback={<p>loading</p>}><Inner /></Loading></main>;
}` },
  { id: "l-shell", kind: "legal", design: "applied", text: `
${LATE}
function Inner() {
  const v = createMemo(async () => await wait());
  return <b>{v()}</b>;
}
export default function App() {
  $CALL;
  return <main><Loading fallback={<p>loading</p>}><Inner /></Loading></main>;
}` },
  { id: "l-fallback", kind: "legal", design: "applied", text: `
${LATE}
function Pending() {
  $CALL;
  return <p>loading</p>;
}
function Inner() {
  const v = createMemo(async () => await wait());
  return <b>{v()}</b>;
}
export default function App() {
  return <main><Loading fallback={<Pending />}><Inner /></Loading></main>;
}` },
  // Below the boundary, but before the pending read: the body runs at shell time, so the declaration applies. The rule's placement
  // claim covers it (an obligation); the runtime says it applies.
  { id: "l-before-read", kind: "legal", design: "applied", text: `
${LATE}
function Inner() {
  const v = createMemo(async () => await wait());
  $CALL;
  return <b>{v()}</b>;
}
export default function App() {
  return <main><Loading fallback={<p>loading</p>}><Inner /></Loading></main>;
}` },
  { id: "l-no-async", kind: "legal", design: "applied", text: `
function Inner() {
  $CALL;
  return <b>1</b>;
}
export default function App() {
  return <main><Loading fallback={<p>loading</p>}><Inner /></Loading></main>;
}` },
  // A source that settles in a microtask: the boundary settles before the shell flush.
  { id: "l-settled-first", kind: "legal", design: "applied", text: `
function Inner() {
  const v = createMemo(async () => 1);
  const n = v();
  $CALL;
  return <b>{n}</b>;
}
export default function App() {
  return <main><Loading fallback={<p>loading</p>}><Inner /></Loading></main>;
}` },
  // `deferStream: true` on a source created outside the boundary holds the shell until it settles (RFC 05). The same option on a
  // source created inside the boundary does not (measured: dropped), which no template here claims.
  { id: "l-defer-outside", kind: "legal", design: "applied", text: `
${LATE}
function Inner(props: { v: () => number }) {
  const n = props.v();
  $CALL;
  return <b>{n}</b>;
}
export default function App() {
  const v = createMemo(async () => await wait(), { deferStream: true });
  return <main><Loading fallback={<p>loading</p>}><Inner v={v} /></Loading></main>;
}` },
];

/** Every program: each template with each declaration. `site` is the declaration's 1-based line in App.tsx. */
export function ssrPrograms({ templates } = {}) {
  const programs = [];
  for (const template of TEMPLATES.filter((t) => !templates || templates.includes(t.id))) {
    for (const [name, call] of Object.entries(CALLS)) {
      const body = template.text.replace("$CALL", call.call(`"1"`)).trimStart();
      const text = `import { Loading, createMemo } from "solid-js";\nimport { ${call.imported} } from "@solidjs/web";\n${body}\n`;
      const lines = text.split("\n");
      const sites = lines.flatMap((line, index) => (line.includes(`${call.imported}(`) ? [index + 1] : []));
      if (sites.length !== 1) throw new Error(`ssr template ${template.id}: ${sites.length} declarations of ${call.imported}`);
      programs.push({ id: `ssr-${template.id}-${name}`, template: template.id, kind: template.kind, design: template.design, call: name, site: sites[0], text });
    }
  }
  return programs;
}

/** The server entry beside App.tsx: what makes the project visibly server-render (the rule's premise). */
export const SERVER_ENTRY = `import { renderToStream } from "@solidjs/web";\nimport App from "./App";\n\nexport function serve() {\n  return renderToStream(() => <App />);\n}\n`;

/** The checker's arguments: a streaming-SSR project on Node, development build (the runtime the host renders with). */
export const SSR_CHECKER_ARGS = ["--format", "json", "--runtime-target", "node", "--runtime-build", "development", "--rendering", "streaming-ssr"];

/** The rules that claim the client runtime (the rules catalog's `claims_client_runtime`): each is silent on a server-only program. */
export const CLIENT_RULES = ["SC1001", "SC2001", "SC2002", "SC2003", "SC2008", "SC5001", "SC6002"];

/**
 * The head's answer against the checker's at the declaration's line. `runtime`: `{ applied }` or `{ failure }`; `findings`: the
 * parsed App.tsx findings (checker.mjs), or null when the checker failed; `typecheck`: `{ ok }`.
 */
export function judgeSsr(program, { runtime, findings, typecheck: checked }) {
  if (checked && !checked.ok) return { outcome: "typed-away", detail: { error: checked.errors?.[0] ?? null } };
  if (!runtime || runtime.failure) return { outcome: "harness", detail: { why: runtime?.failure ?? "no record" } };
  if (!findings) return { outcome: "harness", detail: { why: "the checker failed" } };
  // ADR 0659: the server build has no strict window and no write guard, so a client rule's violation on a server-only program is false.
  const clientClaims = findings.filter((finding) => finding.kind === "violation" && CLIENT_RULES.includes(finding.id));
  if (clientClaims.length) return { outcome: "false-positive", detail: { clientClaims: clientClaims.map((finding) => `${finding.id}@${finding.line}`) } };
  const observed = runtime.applied ? "applied" : "dropped";
  const atSite = findings.filter((finding) => finding.id === "SC7005" && finding.line <= program.site && program.site <= finding.endLine);
  const verdict = atSite.some((finding) => finding.kind === "violation") ? "violation" : atSite.length ? "uncertifiable" : "none";
  const detail = { observed, design: program.design, checker: verdict };
  if (observed !== program.design) return { outcome: "not-as-designed", detail };
  if (observed === "dropped") return { outcome: verdict === "none" ? "false-clean" : "agree", detail };
  return { outcome: verdict === "violation" ? "false-positive" : verdict === "uncertifiable" ? "uncertifiable" : "clean", detail };
}

export const SSR_OUTCOMES = ["agree", "false-clean", "false-positive", "uncertifiable", "clean", "not-as-designed", "typed-away", "harness"];
/** The outcomes that need a look. */
export const SSR_PROBLEMS = ["false-clean", "false-positive", "not-as-designed", "harness"];

/** App.tsx compiled for SSR by the pinned compiler, its types stripped (Node's own pass): an ES module. */
export function compileForSsr(text, install, filename = "App.tsx") {
  const { transform } = createRequire(join(resolve(install), "package.json"))("@solidjs/compiler");
  const { code } = transform(text, { generate: "ssr", dev: true, hydratable: false, filename });
  // Node's pass warns that it is experimental on every first use; the warning is not the run's.
  const emit = process.emitWarning;
  process.emitWarning = () => {};
  try { return stripTypeScriptTypes(code); } finally { process.emitWarning = emit; }
}

/**
 * Renders the programs in one host process (ssr-host.mjs); resolves to a Map id -> `{ status, headers }` or `{ failure }`. A host that
 * dies or prints nothing for a program leaves that program a failure (a harness row), never a head.
 */
export function renderPrograms(cases, { install, timeoutMs = 60_000, deadlineMs = 10_000, runtime = NODE } = {}) {
  return new Promise((done) => {
    const records = new Map();
    const child = spawn(runtime, ["--conditions=development", "--no-warnings", join(here, "ssr-host.mjs"), resolve(install), String(deadlineMs)], { stdio: ["pipe", "pipe", "pipe"] });
    let out = ""; let err = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdout.on("data", (chunk) => { out += chunk; });
    child.stderr.on("data", (chunk) => { err += chunk; });
    child.stdin.on("error", () => {});
    const finish = (why) => {
      clearTimeout(timer);
      for (const line of out.split("\n").filter(Boolean)) {
        try { const row = JSON.parse(line); records.set(row.id, row); } catch { /* a line cut short */ }
      }
      for (const { id } of cases) if (!records.has(id)) records.set(id, { id, failure: `the SSR host ${why}${err ? `: ${err.trim().split("\n").slice(-2).join(" | ").slice(0, 300)}` : ""}` });
      done(records);
    };
    child.on("error", (error) => finish(`did not start (${error.message})`));
    child.on("close", (code, signal) => finish(code === 0 ? "printed no record" : `exited ${code ?? signal}`));
    child.stdin.end(JSON.stringify(cases.map(({ id, module }) => ({ id, module }))));
  });
}

/** Runs the checker on a materialized SSR project (SSR_CHECKER_ARGS); resolves to checker.mjs `checkerResult`'s answer. */
export function runSsrChecker(dir, { bin, typefacts, timeoutMs = 180_000 } = {}) {
  return new Promise((done) => {
    if (!bin || !existsSync(bin)) { done({ ok: false, findings: [], error: `checker binary not found: ${bin}` }); return; }
    const child = spawn(bin, ["--project", join(dir, "tsconfig.json"), ...SSR_CHECKER_ARGS], { cwd: dir, stdio: ["ignore", "pipe", "pipe"], env: { ...checkerEnv({ typefacts }), SOLID_CHECKER_DAEMON: "0" } });
    let stdout = ""; let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", (error) => { clearTimeout(timer); done({ ok: false, findings: [], error: error.message }); });
    child.on("close", (status) => { clearTimeout(timer); done(checkerResult({ status, stdout, stderr }, dir)); });
  });
}

const tally = (rows) => Object.fromEntries(SSR_OUTCOMES.map((outcome) => [outcome, rows.filter((row) => row.outcome === outcome).length]));

export async function runSsr(options) {
  const out = resolve(options.out);
  const bin = options.checkerBin && resolve(options.checkerBin);
  const typefacts = options.typefacts && resolve(options.typefacts);
  const install = resolve(options.install ?? DEFAULT_FAST_INSTALL);
  if (!existsSync(join(install, "node_modules/@solidjs/web/dist/server.dev.js"))) throw new Error(`no pinned install at ${install} (cd scripts/fuzz/install && npm ci)`);
  mkdirSync(out, { recursive: true });
  const programs = ssrPrograms({ templates: options.templates?.split(",") });
  const work = join(install, ".solid-checker-fuzz-ssr", `run-${process.pid}`);
  rmSync(work, { recursive: true, force: true });
  const partial = partialRows(out);
  try {
    // Each program's project (the checker's and tsrs's) and its compiled module, inside the install so both resolve its packages.
    const cases = programs.map((program) => {
      const dir = join(work, program.id);
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "App.tsx"), program.text);
      writeFileSync(join(dir, "server.tsx"), SERVER_ENTRY);
      writeFileSync(join(dir, "tsconfig.json"), tsconfigFor(["server.tsx", "App.tsx"]));
      writeFileSync(join(dir, "App.ssr.mjs"), compileForSsr(program.text, install));
      return { ...program, dir, module: join(dir, "App.ssr.mjs") };
    });
    console.error(`ssr: ${cases.length} programs (${TEMPLATES.length} templates x ${Object.keys(CALLS).length} declarations); rendering`);
    const records = await renderPrograms(cases, { install });
    const tsc = options.tsc ?? resolveTsc();
    const rows = [];
    for (const program of cases) {
      const record = records.get(program.id);
      const runtime = record?.failure ? { failure: record.failure } : record ? { applied: CALLS[program.call].observe(record), status: record.status, header: record.headers?.["x-sc"] ?? null } : null;
      const checked = await typecheck(program.dir, { tsc });
      const checker = checked.ok ? await runSsrChecker(program.dir, { bin, typefacts }) : null;
      const verdict = judgeSsr(program, { runtime, findings: checker?.ok ? checker.findings : checked.ok ? null : [], typecheck: checked });
      const row = { id: program.id, template: program.template, kind: program.kind, call: program.call, site: program.site, outcome: verdict.outcome, detail: { ...verdict.detail, ...(checker && !checker.ok ? { checkerError: checker.error?.slice(0, 300) } : {}) }, findings: (checker?.findings ?? []).map(({ id, kind, line }) => `${id}:${kind}:${line}`) };
      rows.push(row);
      partial.append([row]);
    }
    const report = { mode: "ssr", rule: "SC7005", total: tally(rows), templates: TEMPLATES.map(({ id, kind, design }) => ({ id, kind, design })) };
    writeFileSync(join(out, "ssr.json"), `${JSON.stringify(report, null, 2)}\n`);
    writeFileSync(join(out, "programs.jsonl"), `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`);
    partial.finish();
    console.error(`ssr: ${JSON.stringify(report.total)}`);
    const problems = rows.filter((row) => SSR_PROBLEMS.includes(row.outcome));
    for (const row of problems) console.error(`ssr: ${row.outcome} ${row.id} ${JSON.stringify(row.detail)}`);
    if (rows.some((row) => row.outcome === "false-positive")) { console.error("\nSTOP: a violation where the runtime applied the declaration (a false positive). Report the program(s) above."); return 3; }
    return problems.length ? 1 : 0;
  } finally {
    if (!options.keepWork) rmSync(work, { recursive: true, force: true });
  }
}

export function parseSsrArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (key === "keepWork" || key === "help") { options[key] = true; continue; }
    if (!["out", "checkerBin", "typefacts", "tsc", "install", "templates"].includes(key) || argv[index + 1] === undefined) throw new Error(`ssr.mjs: unknown or empty option ${argv[index]}`);
    options[key] = argv[index += 1];
  }
  return options;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  guardGridExit("ssr");
  try {
    const options = parseSsrArgs(process.argv.slice(2));
    if (options.help || !options.out) { console.error("usage: node scripts/fuzz/ssr.mjs --out <fresh dir> [--checker-bin <solid-checker-rust>] [--typefacts <solid-typefacts>] [--tsc <tsrs>] [--install <dir>] [--templates a,b] [--keep-work]"); process.exitCode = options.help ? 0 : 2; }
    else process.exitCode = await runSsr(options);
  } catch (error) {
    console.error(`ssr: stopped: ${error.message}`);
    process.exitCode = 2;
  }
}
