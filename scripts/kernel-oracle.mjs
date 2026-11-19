#!/usr/bin/env node
// ADR 1120: the kernel oracle. Tests K1 (`solid_facts::kernel::evaluation`) mechanically, with no Solid and no browser: expression
// forms × operands, two levels deep, each evaluated in Node with operands that record every hook a program can write (coercion
// methods, Proxy traps, getters, `Symbol.hasInstance`, a global accessor, prototype setters) and judged against the kernel's answer.
//
//   node scripts/kernel-oracle.mjs [--out <dir>] [--depth2 <n>]
//
// A failure is a case the kernel accepts (with every obligation the oracle can discharge) whose evaluation ran a hook, or whose value
// is not of the kind the kernel names. A refusal is never a failure; the report counts the refusals that ran no hook (precision).
// Obligations are discharged as the IR half does (`solid_reactive_ir::kernel`): `Bound` for a declared operand or an immutable
// global, `Primitive` for an operand declared as a primitive constant. The IR half's own discharge has its unit tests.
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
const out = resolve(option("--out", join(repo, "target", "kernel-oracle")));
const depth2 = Number(option("--depth2", 12000));

// Operands: name → whether the oracle discharges a `Primitive` obligation on it (a `const` of a primitive literal).
const OPERANDS = { p: true, s: true, n: true, h: false, x: false, g: false, k: false, f: false, o: false };
const IMMUTABLE_GLOBALS = ["undefined", "NaN", "Infinity"];
const LEAVES = [...Object.keys(OPERANDS), "G", "undefined", "NaN", "1", "'q'", "null", "`t`", "1n", "[]", "({})", "/r/"];

// Forms: [ts, js] with slots A, B. The TypeScript wrappers are erased for Node.
const FORMS = [
  ...["!", "-", "+", "~", "typeof ", "void "].map((op) => [`${op}(A)`, `${op}(A)`]),
  ["delete (A).a", "delete (A).a"],
  ...["===", "!==", "==", "!=", "<", "<=", ">", ">=", "+", "-", "*", "/", "%", "**", "<<", ">>", ">>>", "|", "^", "&", "in", "instanceof", "&&", "||", "??"].map((op) => [`(A) ${op} (B)`, `(A) ${op} (B)`]),
  ["(A) ? (B) : 1", "(A) ? (B) : 1"],
  ["((A), (B))", "((A), (B))"],
  ["(A).a", "(A).a"], ["(A)['a']", "(A)['a']"], ["(A)[B]", "(A)[B]"], ["(A)?.a", "(A)?.a"],
  ["(A)()", "(A)()"], ["new (A)()", "new (A)()"], ["(A)`t`", "(A)`t`"],
  ["`${A}`", "`${A}`"], ["`x${A}y${B}`", "`x${A}y${B}`"],
  ["[(A), (B)]", "[(A), (B)]"], ["[...(A)]", "[...(A)]"], ["[(A), , (B)]", "[(A), , (B)]"],
  ["({ a: (A) })", "({ a: (A) })"], ["({ ...(A) })", "({ ...(A) })"], ["({ [A]: 1 })", "({ [A]: 1 })"],
  ["({ __proto__: (A) })", "({ __proto__: (A) })"], ["({ ['__proto__']: (A) })", "({ ['__proto__']: (A) })"],
  ["({ m() { return (A); } })", "({ m() { return (A); } })"], ["({ get a() { return (A); } })", "({ get a() { return (A); } })"],
  ["() => (A)", "() => (A)"], ["(function () { return (A); })", "(function () { return (A); })"],
  ["(class { static [A] = 1; })", "(class { static [A] = 1; })"], ["(class extends (A) {})", "(class extends (A) {})"],
  ["((A) as any)", "(A)"], ["(A)!", "(A)"], ["((A) satisfies unknown)", "(A)"], ["(<any>(A))", "(A)"],
];

const fill = ([ts, js], a, b) => [ts.replaceAll("A", a[0]).replaceAll("B", b[0]), js.replaceAll("A", a[1]).replaceAll("B", b[1])];
// `A`/`B` inside an operand name would be replaced twice: operand names are lower case and forms use upper-case slots only.
const leaves = LEAVES.map((leaf) => [leaf, leaf]);
const level1 = [];
for (const form of FORMS) {
  const slots = form[0].includes("B") ? leaves : [["p", "p"]];
  for (const a of leaves) for (const b of slots) level1.push(fill(form, a, b));
}
// Level 2: a deterministic spread of level-1 expressions in slot A, a leaf in slot B.
const level2 = [];
let seed = 1120;
const next = () => ((seed = (seed * 1103515245 + 12345) >>> 0) % 2 ** 31);
for (let i = 0; i < depth2; i += 1) {
  const form = FORMS[next() % FORMS.length];
  level2.push(fill(form, level1[next() % level1.length], leaves[next() % leaves.length]));
}
const cases = [...leaves, ...level1, ...level2].filter(([ts]) => !ts.includes("\n"));

// The kernel's verdicts, from the ignored Rust test.
mkdirSync(out, { recursive: true });
const input = join(out, "cases.txt");
const output = join(out, "verdicts.jsonl");
writeFileSync(input, `${cases.map(([ts]) => `(${ts})`).join("\n")}\n`);
execFileSync("cargo", ["+1.99.1", "test", "--manifest-path", join(repo, "rust/Cargo.toml"), "-p", "solid-facts", "--lib", "kernel::tests::kernel_oracle", "--", "--ignored", "--exact", "--quiet"], {
  stdio: ["ignore", "ignore", "inherit"],
  env: { ...process.env, SOLID_KERNEL_ORACLE_IN: input, SOLID_KERNEL_ORACLE_OUT: output },
});
const verdicts = readFileSync(output, "utf8").trimEnd().split("\n").map((line) => JSON.parse(line));
if (verdicts.length !== cases.length) throw new Error(`kernel answered ${verdicts.length} of ${cases.length} cases`);

// Hostile operands and environment. `hits` counts every hook that ran while a case was armed.
let armed = false;
// A Set, never an array: the array index setters installed for a case would swallow `push`.
const hits = new Set();
const hit = (what) => { if (armed) hits.add(what); };
const hostile = () => {
  const coerce = { [Symbol.toPrimitive]() { hit("toPrimitive"); return 1; }, valueOf() { hit("valueOf"); return 1; }, toString() { hit("toString"); return "h"; }, toJSON() { hit("toJSON"); return 1; } };
  const proxy = new Proxy(function target() {}, Object.fromEntries(["get", "set", "has", "deleteProperty", "ownKeys", "getOwnPropertyDescriptor", "defineProperty", "getPrototypeOf", "setPrototypeOf", "isExtensible", "preventExtensions", "apply", "construct"].map((trap) => [trap, (...rest) => { hit(`proxy.${trap}`); return Reflect[trap](...rest); }])));
  const getters = Object.defineProperties({}, Object.fromEntries(["a", "length", "then", "0", "__proto__x"].map((key) => [key, { get() { hit(`get.${key}`); return 1; }, enumerable: true }])));
  class Hostile { static [Symbol.hasInstance]() { hit("hasInstance"); return false; } }
  const fn = function () { hit("call"); return 1; };
  const plain = { a: 1 };
  return { p: 1, s: "s", n: 0, h: coerce, x: proxy, g: getters, k: Hostile, f: fn, o: plain };
};
Object.defineProperty(globalThis, "G", { get() { hit("global.G"); return 1; }, configurable: true });
// Installed for the cases only: Node's own code assigns array indexes, which the setters would swallow.
const PROTOTYPE_SETTERS = [[Array.prototype, "0"], [Array.prototype, "1"], [Object.prototype, "a"], [Object.prototype, "m"]];
const installSetters = () => { for (const [owner, key] of PROTOTYPE_SETTERS) Object.defineProperty(owner, key, { set() { hit(`setter.${key}`); }, configurable: true }); };
const removeSetters = () => { for (const [owner, key] of PROTOTYPE_SETTERS) delete owner[key]; };

const names = Object.keys(OPERANDS);
// Reads descriptors only: a getter of the value is code, never run here.
const kindOf = (value) => {
  if (value === null || (typeof value !== "object" && typeof value !== "function")) return "Primitive";
  if (typeof value === "function") return "Code";
  const own = Object.getOwnPropertyDescriptors(value);
  return Object.values(own).some((d) => d.get || d.set || typeof d.value === "function") ? "Code" : "Data";
};
const discharged = (verdict) => verdict.obligations.every(([name, need]) => (need === "Bound" ? name in OPERANDS || IMMUTABLE_GLOBALS.includes(name) : OPERANDS[name] === true));
const report = { cases: cases.length, accepted: 0, acceptedConditional: 0, refused: 0, refusedQuiet: 0, failures: [] };
for (let i = 0; i < cases.length; i += 1) {
  const [ts, js] = cases[i];
  const verdict = verdicts[i];
  let compiled;
  try {
    compiled = new Function(...names, `return (${js});`);
  } catch {
    // Not JavaScript (a syntax error the TypeScript parser tolerates): the kernel must not accept it.
    if (verdict) report.failures.push({ ts, why: "accepted, but Node rejects the syntax" });
    continue;
  }
  const operands = hostile();
  const operandValues = names.map((name) => operands[name]);
  hits.clear();
  let value;
  let threw = false;
  installSetters();
  armed = true;
  try {
    value = compiled(...operandValues);
  } catch {
    threw = true;
  } finally {
    armed = false;
    removeSetters();
  }
  if (!verdict) {
    report.refused += 1;
    if (!hits.size) report.refusedQuiet += 1;
    continue;
  }
  if (!discharged(verdict)) {
    report.acceptedConditional += 1;
    continue;
  }
  report.accepted += 1;
  if (hits.size) report.failures.push({ ts, why: `ran ${[...hits].join(", ")}`, verdict });
  else if (!threw && verdict.kind !== "Operand" && kindOf(value) !== verdict.kind && !(verdict.kind === "Data" && kindOf(value) === "Primitive")) report.failures.push({ ts, why: `value is ${kindOf(value)}, kernel says ${verdict.kind}` });
}
// Canaries: cases that must have run a hook. If one did not, the harness is blind (an earlier version recorded hooks in an array
// whose index setter it had replaced) and its zero failures mean nothing.
const CANARIES = { "(x).a": "proxy.get", "`${h}`": "toPrimitive", "(G) === (p)": "global.G", "(p) instanceof (k)": "hasInstance", "(g).a": "get.a", "(f)()": "call", "(p) in (x)": "proxy.has" };
const blind = [];
for (const [ts, hook] of Object.entries(CANARIES)) {
  const compiled = new Function(...names, `return (${ts});`);
  const operands = hostile();
  hits.clear();
  installSetters();
  armed = true;
  try { compiled(...names.map((name) => operands[name])); } catch {} finally { armed = false; removeSetters(); }
  if (!hits.has(hook)) blind.push(`${ts} did not run ${hook}`);
}
const setterCanary = new Function("return (() => { const a = []; a[0] = 1; })()");
hits.clear(); installSetters(); armed = true;
try { setterCanary(); } finally { armed = false; removeSetters(); }
if (!hits.has("setter.0")) blind.push("an index assignment did not run the Array.prototype setter");
if (blind.length) {
  console.log(`kernel-oracle: BLIND: ${blind.join("; ")}`);
  process.exit(2);
}
writeFileSync(join(out, "report.json"), `${JSON.stringify(report, null, 1)}\n`);
console.log(`kernel-oracle: ${report.cases} cases; accepted ${report.accepted} (+${report.acceptedConditional} on undischarged obligations), refused ${report.refused} (${report.refusedQuiet} ran no hook); failures ${report.failures.length}`);
for (const failure of report.failures.slice(0, 40)) console.log(`  FAIL ${failure.ts}: ${failure.why}`);
process.exit(report.failures.length ? 1 : 0);
