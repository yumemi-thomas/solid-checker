#!/usr/bin/env node
// The witness oracle for the rules whose defect is a UI that stops updating, not a diagnostic of its own (Fable's report R5, Astra's
// proposal 3): no-destructure (SC1003), components-return-once (SC1004), uncalled-accessor (SC1005), reactive-handler-frozen
// (SC1007); and a static specification check for the style rules prefer-for (SC8014) and prefer-show (SC8015), which no runtime can
// decide. Never a fake runtime diagnostic: the evidence is the page's own text.
//
// Each template is a pair: the rule's defect form and its live twin, every one a probe (replay.mjs `probeOf`: App.tsx and probe.json
// `{ witness: { selector, text }, steps }`). A button writes a signal (idempotently: the oracle clicks every element twice before the
// steps); `#w` shows a value derived from it in the suspect position. The witness text is the updated value, so Chrome's page says
// which one the program is: `landed` (the UI followed the write: live) or `not-landed` (stale). Judged per program for its rule:
//
//   agree           stale, and a violation of the rule
//   recall-gap      stale, and only an uncertifiable finding of the rule (or a withholding)
//   false-clean     stale, and nothing of the rule
//   false-positive  live, and a violation of the rule (stop and report: the witness shows the UI updated)
//   uncertifiable   live, and only an uncertifiable finding
//   clean           live, and nothing of the rule
//   inconclusive    the witness is absent, or Chrome could not run the page
//   tsc-overlap     tsrs rejects the program and a violation covers the rejected code (tsc-overlap.mjs); `typed-away` otherwise
// `expect` is each template's design (stale or live); a program Chrome shows the other way is listed (`unexpected`), a template to fix.
// The style rules: `flag` templates must carry the rule's violation and `silent` ones none (`spec-agree`, `spec-miss`, `spec-flagged`).
//
//   node scripts/fuzz/witness.mjs --out <fresh dir> [--confirm-with-chrome] [--rules a,b] [--checker-bin …] [--typefacts …] [--tsc …]
// Without Chrome the stale rules are judged by `expect` alone (`static-agree`, `static-miss`, `static-flagged`): candidates only.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { assertFindingsMapped, materializeForChecker, runChecker } from "./checker.mjs";
import { withholdingRules } from "./codes.mjs";
import { typedAwayVerdict } from "./tsc-overlap.mjs";
import { requireTypeChecker, resolveTsc, typecheck } from "./typecheck.mjs";

const here = dirname(fileURLToPath(import.meta.url));

/** The rules the witness decides (the UI's text), and the style rules the specification check decides. */
export const WITNESS_RULES = ["no-destructure", "components-return-once", "uncalled-accessor", "reactive-handler-frozen"];
export const SPEC_RULES = ["prefer-for", "prefer-show"];

const app = (imports, body) => `import { ${imports.join(", ")} } from "solid-js";\n\n${body.trim()}\n`;
// A parent with `n` (1, then 2 on #b) and a child reading it through a prop `v`.
const parent = (kid, extra = "") => `
export function C1() {
  const [n, setN] = createSignal(1);${extra}
  return (
    <div>
      <Kid v={n()} />
      <button id="b" onClick={() => setN(2)}>go</button>
    </div>
  );
}

${kid}`;
const kidType = "{ v: number }";

/**
 * The templates: `{ rule, name, expect, text, witness?, steps? }`. A stale-rule template's witness defaults to `#w` showing "2" after
 * a click on `#b`. Each pair's live twin differs only in the suspect position.
 */
export const TEMPLATES = [
  // no-destructure: a destructured prop is read once, in the body, so the child shows the first value.
  { rule: "no-destructure", name: "param", expect: "stale", text: app(["createSignal"], parent(`function Kid({ v }: ${kidType}) {\n  return <p id="w">{v}</p>;\n}`)) },
  { rule: "no-destructure", name: "param-default", expect: "stale", text: app(["createSignal"], parent(`function Kid({ v = 0 }: Partial<${kidType}>) {\n  return <p id="w">{v}</p>;\n}`)) },
  { rule: "no-destructure", name: "param-rename", expect: "stale", text: app(["createSignal"], parent(`function Kid({ v: value }: ${kidType}) {\n  return <p id="w">{value}</p>;\n}`)) },
  { rule: "no-destructure", name: "body", expect: "stale", text: app(["createSignal"], parent(`function Kid(props: ${kidType}) {\n  const { v } = props;\n  return <p id="w">{v}</p>;\n}`)) },
  { rule: "no-destructure", name: "body-rest", expect: "stale", text: app(["createSignal"], parent(`function Kid(props: ${kidType} & { x?: number }) {\n  const { x, ...rest } = props;\n  return <p id="w">{rest.v}{x}</p>;\n}`)) },
  { rule: "no-destructure", name: "member", expect: "live", text: app(["createSignal"], parent(`function Kid(props: ${kidType}) {\n  return <p id="w">{props.v}</p>;\n}`)) },
  { rule: "no-destructure", name: "memo", expect: "live", text: app(["createMemo", "createSignal"], parent(`function Kid(props: ${kidType}) {\n  const v = createMemo(() => {\n    const { v } = props;\n    return v;\n  });\n  return <p id="w">{v()}</p>;\n}`)) },
  { rule: "no-destructure", name: "jsx", expect: "live", text: app(["createSignal"], parent(`function Kid(props: ${kidType}) {\n  return <p id="w">{(() => {\n    const { v } = props;\n    return v;\n  })()}</p>;\n}`)) },
  { rule: "no-destructure", name: "omit", expect: "live", text: app(["createSignal", "omit"], parent(`function Kid(props: ${kidType} & { x?: number }) {\n  const rest = omit(props, "x");\n  return <p id="w">{rest.v}</p>;\n}`)) },
  // components-return-once: a component body runs once, so a conditional return picks its branch for good.
  { rule: "components-return-once", name: "early-return", expect: "stale", text: app(["createSignal"], parent(`function Kid(props: ${kidType}) {\n  if (props.v === 2) return <p id="w">2</p>;\n  return <p id="w">1</p>;\n}`)) },
  { rule: "components-return-once", name: "ternary-return", expect: "stale", text: app(["createSignal"], parent(`function Kid(props: ${kidType}) {\n  return props.v === 2 ? <p id="w">2</p> : <p id="w">1</p>;\n}`)) },
  { rule: "components-return-once", name: "early-return-signal", expect: "stale", text: app(["createSignal"], `
export function C1() {
  const [n, setN] = createSignal(1);
  if (n() === 2) return <p id="w">2</p>;
  return (
    <div>
      <p id="w">1</p>
      <button id="b" onClick={() => setN(2)}>go</button>
    </div>
  );
}`) },
  { rule: "components-return-once", name: "jsx-ternary", expect: "live", text: app(["createSignal"], parent(`function Kid(props: ${kidType}) {\n  return <>{props.v === 2 ? <p id="w">2</p> : <p id="w">1</p>}</>;\n}`)) },
  { rule: "components-return-once", name: "show", expect: "live", text: app(["Show", "createSignal"], parent(`function Kid(props: ${kidType}) {\n  return <Show when={props.v === 2} fallback={<p id="w">1</p>}><p id="w">2</p></Show>;\n}`)) },
  // uncalled-accessor: the accessor itself where its value was meant (the positions TypeScript accepts).
  { rule: "uncalled-accessor", name: "template", expect: "stale", witness: { selector: "#w", text: "n2" }, text: app(["createSignal"], `
export function C1() {
  const [n, setN] = createSignal(1);
  return (
    <div>
      <p id="w">{\`n\${n}\`}</p>
      <button id="b" onClick={() => setN(2)}>go</button>
    </div>
  );
}`) },
  { rule: "uncalled-accessor", name: "concat", expect: "stale", witness: { selector: "#w", text: "n2" }, text: app(["createSignal"], `
export function C1() {
  const [n, setN] = createSignal(1);
  return (
    <div>
      <p id="w">{"n" + n}</p>
      <button id="b" onClick={() => setN(2)}>go</button>
    </div>
  );
}`) },
  { rule: "uncalled-accessor", name: "not", expect: "stale", witness: { selector: "#w", text: "true" }, text: app(["createSignal"], `
export function C1() {
  const [n, setN] = createSignal(1);
  return (
    <div>
      <p id="w">{String(!n)}</p>
      <button id="b" onClick={() => setN(0)}>go</button>
    </div>
  );
}`) },
  { rule: "uncalled-accessor", name: "template-called", expect: "live", witness: { selector: "#w", text: "n2" }, text: app(["createSignal"], `
export function C1() {
  const [n, setN] = createSignal(1);
  return (
    <div>
      <p id="w">{\`n\${n()}\`}</p>
      <button id="b" onClick={() => setN(2)}>go</button>
    </div>
  );
}`) },
  { rule: "uncalled-accessor", name: "not-called", expect: "live", witness: { selector: "#w", text: "true" }, text: app(["createSignal"], `
export function C1() {
  const [n, setN] = createSignal(1);
  return (
    <div>
      <p id="w">{String(!n())}</p>
      <button id="b" onClick={() => setN(0)}>go</button>
    </div>
  );
}`) },
  // reactive-handler-frozen: a handler expression is evaluated once, so a handler read from a signal stays the first one. #s swaps the
  // handler, #c runs it; the witness is the second handler's text.
  ...[["snapshot", "stale", "onClick={h().run}"], ["arrow", "live", "onClick={() => h().run()}"]].map(([name, expect, handler]) => ({ rule: "reactive-handler-frozen", name, expect,
    witness: { selector: "#w", text: "b" }, steps: [{ action: "click", selector: "#s" }, { action: "click", selector: "#c" }], text: app(["createSignal"], `
export function C1() {
  const [w, setW] = createSignal("-");
  const [h, setH] = createSignal({ run: () => { setW("a"); } });
  return (
    <div>
      <p id="w">{w()}</p>
      <button id="s" onClick={() => setH({ run: () => { setW("b"); } })}>swap</button>
      <button id="c" ${handler}>run</button>
    </div>
  );
}`) })),
  ...[["prop-snapshot", "stale", "run={h().run}"], ["prop-stable", "live", "run={() => h().run()}"]].map(([name, expect, prop]) => ({ rule: "reactive-handler-frozen", name, expect,
    witness: { selector: "#w", text: "b" }, steps: [{ action: "click", selector: "#s" }, { action: "click", selector: "#c" }], text: app(["createSignal"], `
export function C1() {
  const [w, setW] = createSignal("-");
  const [h, setH] = createSignal({ run: () => { setW("a"); } });
  return (
    <div>
      <p id="w">{w()}</p>
      <button id="s" onClick={() => setH({ run: () => { setW("b"); } })}>swap</button>
      <Kid ${prop} />
    </div>
  );
}

function Kid(props: { run: () => void }) {
  return <button id="c" onClick={props.run}>run</button>;
}`) })),
  // prefer-for, prefer-show: a style the runtime cannot decide; the rule page's claim is the specification.
  { rule: "prefer-for", name: "map-signal", expect: "flag", text: app(["createSignal"], `
export function C1() {
  const [items] = createSignal([1, 2]);
  return <ul>{items().map((x) => <li>{x}</li>)}</ul>;
}`) },
  { rule: "prefer-for", name: "for", expect: "silent", text: app(["For", "createSignal"], `
export function C1() {
  const [items] = createSignal([1, 2]);
  return <ul><For each={items()}>{(x) => <li>{x}</li>}</For></ul>;
}`) },
  { rule: "prefer-for", name: "map-static", expect: "silent", text: app(["createSignal"], `
export function C1() {
  const [n] = createSignal(1);
  return <ul>{[1, 2].map((x) => <li>{x}</li>)}{n()}</ul>;
}`) },
  { rule: "prefer-show", name: "and-signal", expect: "flag", text: app(["createSignal"], `
export function C1() {
  const [on] = createSignal(true);
  return <div>{on() && <p>x</p>}</div>;
}`) },
  { rule: "prefer-show", name: "show", expect: "silent", text: app(["Show", "createSignal"], `
export function C1() {
  const [on] = createSignal(true);
  return <div><Show when={on()}><p>x</p></Show></div>;
}`) },
  { rule: "prefer-show", name: "and-static", expect: "silent", text: app(["createSignal"], `
export function C1() {
  const [n] = createSignal(1);
  return <div>{true && <p>x</p>}{n()}</div>;
}`) },
];

/** The probe directories of the templates: `<out>/<rule>/<name>/` with App.tsx and probe.json (the template's rule and design too). */
export function writeProbes(out, templates = TEMPLATES) {
  const dirs = [];
  for (const template of templates) {
    const dir = join(out, template.rule, template.name);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "App.tsx"), template.text);
    const witness = template.witness ?? { selector: "#w", text: "2" };
    const steps = template.steps ?? [{ action: "click", selector: "#b" }];
    writeFileSync(join(dir, "probe.json"), `${JSON.stringify({ witness, steps, rule: template.rule, expect: template.expect, note: "witness.mjs" })}\n`);
    dirs.push({ dir, template });
  }
  return dirs;
}

/**
 * One program's verdict for its rule: from Chrome's witness (`landed`, `not-landed`, `absent`) when there is one, else from the
 * template's design (`static-*`); the style rules by the specification.
 */
export function judgeWitness({ rule, expect, findings, witness, typed }) {
  if (typed && typed.ok === false) return typedAwayVerdict({ typed, checker: { ok: true, findings }, region: [], rules: [rule] });
  const mine = findings.filter((finding) => finding.rule === rule);
  const violation = mine.some((finding) => finding.kind === "violation");
  const declined = !violation && (mine.length > 0 || findings.some((finding) => withholdingRules.includes(finding.rule)));
  if (SPEC_RULES.includes(rule)) return { outcome: expect === "flag" ? (violation ? "spec-agree" : "spec-miss") : violation ? "spec-flagged" : "spec-agree" };
  const state = witness === "landed" ? "live" : witness === "not-landed" ? "stale" : witness === undefined ? null : "absent";
  if (state === "absent") return { outcome: "inconclusive", detail: { why: "the witness is absent" } };
  const observed = state ?? expect;
  if (observed === null) return { outcome: "inconclusive", detail: { why: "no design and no Chrome" } };
  const prefix = state ? "" : "static-";
  const unexpected = state && expect && state !== expect ? { unexpected: `designed ${expect}, Chrome shows ${state}` } : {};
  const outcome = observed === "stale" ? (violation ? "agree" : declined ? "recall-gap" : "false-clean") : violation ? "false-positive" : declined ? "uncertifiable" : "clean";
  const named = !prefix ? outcome : { agree: "static-agree", "recall-gap": "static-gap", "false-clean": "static-miss", "false-positive": "static-flagged", uncertifiable: "static-uncertifiable", clean: "static-clean" }[outcome];
  return { outcome: named, detail: { state: state ?? `designed ${expect}`, findings: mine.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), ...unexpected } };
}

const PROBLEMS = ["false-positive", "false-clean", "tsc-overlap", "spec-flagged", "spec-miss"];

async function main(argv) {
  const value = (name) => { const at = argv.indexOf(name); return at < 0 ? undefined : argv[at + 1]; };
  const out = value("--out");
  if (!out) throw new Error("usage: node scripts/fuzz/witness.mjs --out <fresh dir> [--confirm-with-chrome] [--rules a,b] [--checker-bin …] [--typefacts …] [--tsc …]");
  const root = resolve(out);
  if (existsSync(root)) throw new Error(`use a fresh --out path (${root} exists)`);
  const bin = value("--checker-bin") ?? process.env.SOLID_CHECKER_NATIVE_BIN;
  const typefacts = value("--typefacts") ?? process.env.SOLID_TYPEFACTS_BIN;
  const tsc = resolveTsc(value("--tsc"));
  requireTypeChecker(tsc);
  const rules = value("--rules")?.split(",");
  const probes = writeProbes(join(root, "probes"), TEMPLATES.filter((template) => !rules || rules.includes(template.rule)));
  // The checker on each probe, every rule (the style rules included), and the type check.
  const checked = new Map();
  for (const { dir, template } of probes) {
    const built = { id: `wt-${template.rule}-${template.name}`, files: { "App.tsx": template.text } };
    const work = materializeForChecker(built, DEFAULT_ORACLE_INSTALL, built.id);
    const typed = await typecheck(work, { tsc });
    const checker = await runChecker(work, { bin, typefacts });
    rmSync(work, { recursive: true, force: true });
    checked.set(dir, { typed, checker });
  }
  assertFindingsMapped();
  // Chrome: replay.mjs runs every probe and reads its witness (replay.jsonl `chrome.witness`).
  const witnesses = new Map();
  if (argv.includes("--confirm-with-chrome")) {
    const replayOut = join(root, "replay");
    const args = [join(here, "replay.mjs"), "--out", replayOut, join(root, "probes"), "--confirm-with-chrome", ...["--checker-bin", "--typefacts", "--tsc"].flatMap((flag) => (value(flag) ? [flag, value(flag)] : []))];
    const run = spawnSync(process.execPath, args, { stdio: ["ignore", "inherit", "inherit"] });
    if (run.status !== 0 && run.status !== 3) throw new Error(`replay.mjs exited ${run.status}`);
    for (const line of readFileSync(join(replayOut, "replay.jsonl"), "utf8").split("\n").filter(Boolean)) {
      const row = JSON.parse(line);
      witnesses.set(resolve(row.dir), row.chrome?.witness ?? (row.chrome ? "absent" : undefined));
    }
  }
  const rows = probes.map(({ dir, template }) => {
    const { typed, checker } = checked.get(dir);
    const base = { rule: template.rule, name: template.name, expect: template.expect, dir };
    if (!checker.ok) return { ...base, outcome: "harness", detail: { why: String(checker.error).slice(0, 200) } };
    const verdict = judgeWitness({ rule: template.rule, expect: template.expect, findings: checker.findings, witness: witnesses.has(resolve(dir)) ? witnesses.get(resolve(dir)) : undefined, typed });
    return { ...base, ...verdict, findings: checker.findings.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`) };
  });
  const tally = {};
  for (const row of rows) tally[row.outcome] = (tally[row.outcome] ?? 0) + 1;
  writeFileSync(join(root, "witness.jsonl"), `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`);
  writeFileSync(join(root, "witness.json"), `${JSON.stringify({ programs: rows.length, tally, problems: rows.filter((row) => PROBLEMS.includes(row.outcome)).map((row) => `${row.rule}/${row.name}: ${row.outcome}`), unexpected: rows.filter((row) => row.detail?.unexpected).map((row) => `${row.rule}/${row.name}: ${row.detail.unexpected}`) }, null, 2)}\n`);
  for (const row of rows) console.error(`witness: ${row.rule}/${row.name}: ${row.outcome}${row.detail?.unexpected ? ` (${row.detail.unexpected})` : ""} ${row.findings?.filter((f) => f.startsWith(row.rule)).join(" ") ?? ""}`);
  console.error(`witness: ${JSON.stringify(tally)}`);
  // A false positive the witness shows is the stop-and-report case, as Chrome's is in the grids.
  return rows.some((row) => row.outcome === "false-positive") ? 3 : rows.some((row) => PROBLEMS.includes(row.outcome)) ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).then((code) => { process.exitCode = code; }, (error) => { console.error(`witness: ${error.message}`); process.exitCode = 2; });
}
