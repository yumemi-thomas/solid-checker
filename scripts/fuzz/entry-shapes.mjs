// The entry shapes (ADR 0962): the site reached through an entry the checker's walk may take for the only one. A researcher's five Chrome false cleans on main
// 29b3f9312, each a program Solid reports and the checker certified clean:
//
//   an action's step body that is a generator reached INDIRECTLY (SC2006, FLUSH_IN_ACTION)
//     ae-helper        misuse  a generator literal handed to a project helper that makes the action: `const run = (g) => action(g); run(function* () { flush(); })`
//     ae-helper-named  misuse  the same, the generator handed by name
//     ae-package       misuse  the same through a dependency whose source the checker reads: `export function run(cb) { return action(cb); }`
//     ae-delegate      misuse  the action's generator delegates, `yield* inner()`, and `flush()` is inside `inner`
//     ae-*-plain       legal   each twin differs only in what makes the generator an action's step: the helper or the package returns it (no action is made) and a handler
//                              steps it, or the delegating generator is a plain one a handler steps. `flush()` in a handler is no defect.
//
//   a function ENTERED TWICE under different contexts, the second entry the forbidden one (SC3001, `leaf-owner-forbidden-call`)
//     tw-<layer>-<spelling>-<order>-<host>   the site is the body of `g`, a function the host's callback declares and enters twice, once under `layer` (a layer the
//                              runtime tolerates for the code: `runWithOwner(null, g)` for a primitive, `isPending(g)` for a `flush`: primitive-layer-shapes.mjs
//                              PL_MEASURED) and once directly, where the code is reported
//       spelling   call   `isPending(g); g();`
//                  flag   `make(true); make(false);` (the helper chooses the entry by its argument: `if (p0) { isPending(g); } else { g(); }`)
//       order      first  the tolerant entry first, so the second entry is the forbidden one; last  the forbidden entry first
//       host       tracked (`createTrackedEffect`) or settled (`onSettled`): the two leaf hosts the leaf-owner codes are reported in
//       place      (none) the entries are statements of the host's callback; `in` they are the body of a helper `both` declared in that callback and called there; `out` the same with
//                  `g` and `both` declared in the component's body. A walk of a helper that takes a function it has entered for a first one (the checker's `visited` set
//                  of cleanup.rs) never enters it a second time, so the second entry is what the checker misses there, in this place only: the entries in the host's own text it reads apart.
//     tw-<layer>-<spelling>-both-<host>    legal twin: both entries tolerant, so Chrome reports nothing and no entry is forbidden
//
//   a generator STEPPED inside a host callback (ADR 0961, F2: Chrome fires FLUSH_IN_EFFECT_CALLBACK and PRIMITIVE_IN_EFFECT_CALLBACK at the site of a generator an
//   effect's apply iterates; main 29b3f9312 certified it clean. R1: Chrome fires FLUSH_IN_ACTION at the site of a generator an action's step drains with
//   `Array.from`, `.next()` or `new Set`; 588affbf3 certified it clean)
//     ge-<step>-<host>   the site is the body of `g`, a generator the component declares, and the host's callback steps it
//       step   forof   `for (const s of g()) void s`       spread  `void [...g()]`      next  `const it = g(); it.next()`      yield  `for (const s of o()) …` with `function* o() { yield* g(); }`
//              from    `void Array.from(g())`              direct  `g().next()`          set   `void new Set(g())`            stored `const it = g(); for (const s of it) void s`
//              anext   `void g().next()` of an async generator (its body runs up to the first await or yield at once)
//       host   apply   an effect's apply (the codes of the effect phase)    memo   a memo's compute     effect   an effect's compute (the write code)
//              action  an action's step body, a generator literal written as the action's argument (FLUSH_IN_ACTION)
//              tracked / settled  a leaf scope's callback (`createTrackedEffect`, `onSettled`: the leaf codes, batch-10 addendum 12 group B)
//     ge-idle-<host>     legal twin: the host's callback calls `g();` and discards the iterator (none of the body runs there) and a button's handler steps it with a `for…of`, where the site is legal
//
//   a site in a PARAMETER INITIALIZER of a function the host's callback calls (ADR 0961, PD: Chrome fires FLUSH_IN_ACTION at the flush of `function h(x = flush())` an
//   action's step calls, and of a closure or a helper the default reaches; 588affbf3 certified it clean: every net asked about code in a function BODY)
//     pd-<form>-<host>   the site is in the default of `h`'s parameter, and the host's callback calls `h()`
//       form   value    the site call is the default itself, `h(p0: unknown = flush())` (an IIFE where the site is more than one expression)
//              iife     the default is an IIFE holding the site, `h(p0: unknown = (() => { flush(); return 1; })())`
//              closure  the default is a closure holding the site, `h(p0: () => void = () => { flush(); }) { p0(); }`
//              helper   the default calls a function that holds the site, `h(p0: unknown = g())`
//              destructured  the default is inside a destructuring pattern, `h({ p0 = flush() }: { p0?: unknown } = {})`
//              ctor     a class's constructor takes it, `class K8 { constructor(p0: unknown = flush()) {} }` and `new K8()`
//       host   action  an action's step body (FLUSH_IN_ACTION)    apply  an effect's apply (the effect phase's codes)    tracked  `createTrackedEffect`    settled  `onSettled` (the leaf codes)
//              await   an async memo's continuation, after its await (UNTRACKED_READ_AFTER_AWAIT)
//     pd-passed-<host>   legal twin: the host calls `h(1)`: the argument is passed, so the default never runs
//     pd-idle-<host>     legal twin: the host calls nothing, and a button's handler calls `h()`, where the site is legal
//
// Recall contexts only, base shapes (misuse.mjs BASE_ONLY_CONTEXTS): the question is how the site is entered, not the spelling of the site. The census does not cross them
// with other codes (matrix.mjs CENSUS_SKIPS): they are the cells of the codes that name them, as the `pl-*` ones.
import { awaitStmt, bareParam, button, call, child, contain, ctl, decl, domExpr, expr, fn, iife, num, param, prim, ref, ret } from "./model.mjs";

/** The helper `ae-helper` hands the generator to (a project's `run`): it makes the action. */
const RUN_ACTION = "(g: () => Generator<unknown, void, any>) => action(g)";
/** Its twin: it makes no action, it returns the generator function. */
const RUN_PLAIN = "(g: () => Generator<unknown, void, any>) => g";

/** The dependency of `ae-package`: a package the checker reads from source, whose one export makes the action (`ae-package-plain`: returns the generator function, makes none). */
export const ACTION_PACKAGE = "fz-act";
export const ACTION_PACKAGE_CALLEE = "runAction";
export const actionPackageFiles = (plain) => ({
  "package.json": `${JSON.stringify({ name: ACTION_PACKAGE, version: "1.0.0", type: "module", peerDependencies: { "solid-js": "^2.0.0-rc.21" }, exports: { ".": { types: "./index.d.ts", default: "./index.js" } } }, null, 2)}\n`,
  "index.js": plain ? `export function ${ACTION_PACKAGE_CALLEE}(cb) {\n  return cb;\n}\n` : `import { action } from "solid-js";\n\nexport function ${ACTION_PACKAGE_CALLEE}(cb) {\n  return action(cb);\n}\n`,
  "index.d.ts": `export declare function ${ACTION_PACKAGE_CALLEE}(cb: () => Generator<unknown, void, any>): ${plain ? "() => Generator<unknown, void, any>" : "() => Promise<void>"};\n`,
});

export const ENTRY_ACTION_CONTEXTS = {
  "ae-helper": "misuse", "ae-helper-named": "misuse", "ae-package": "misuse", "ae-delegate": "misuse",
  "ae-helper-plain": "legal", "ae-helper-named-plain": "legal", "ae-package-plain": "legal", "ae-delegate-plain": "legal",
};

/** The tolerant layer of each code the twice-entered shapes are placed for, and what the layer is called in a context name. */
export const TW_LAYERS = { PRIMITIVE_IN_FORBIDDEN_SCOPE: "owner-null", FLUSH_IN_TRACKED_QUEUE_CALLBACK: "ispending" };
export const TW_SPELLINGS = ["call", "flag"];
export const TW_ORDERS = ["first", "last"];
export const TW_HOSTS = ["tracked", "settled"];
export const TW_PLACES = ["", "in", "out"];
const LAYER_NAMES = [...new Set(Object.values(TW_LAYERS))];

const TW_NAMES = LAYER_NAMES.flatMap((layer) => TW_SPELLINGS.flatMap((spelling) => TW_HOSTS.flatMap((host) => TW_PLACES.flatMap((place) => [...TW_ORDERS, "both"].map((order) => `tw-${layer}-${spelling}-${order}-${host}${place ? `-${place}` : ""}`)))));
const TW_PATTERN = new RegExp(`^tw-(${LAYER_NAMES.join("|")})-(${TW_SPELLINGS.join("|")})-(${[...TW_ORDERS, "both"].join("|")})-(${TW_HOSTS.join("|")})(?:-(in|out))?$`);
/** `{ layer, spelling, order, host, place }` of a `tw-*` context name (`place` "" for the entries in the host's own text), or null. */
export const twParts = (context) => { const match = TW_PATTERN.exec(context); return match ? { layer: match[1], spelling: match[2], order: match[3], host: match[4], place: match[5] ?? "" } : null; };

/** ADR 0961 (F2, R1): the generator-stepped family. The effect phase's codes in an apply, the write code in a compute (a memo's or an effect's), FLUSH_IN_ACTION in an action's step. */
export const GE_STEPS = ["forof", "spread", "next", "yield", "from", "direct", "set", "stored", "anext"];
export const GE_APPLY_CODES = ["FLUSH_IN_EFFECT_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK"];
export const GE_COMPUTE_CODES = ["REACTIVE_WRITE_IN_OWNED_SCOPE"];
export const GE_ACTION_CODES = ["FLUSH_IN_ACTION"];
/** The leaf scopes (`createTrackedEffect`, `onSettled`) and the codes reported in them (batch-10 addendum 12, group B). */
export const GE_LEAF_CODES = ["FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "CLEANUP_IN_FORBIDDEN_SCOPE"];
export const GE_LEAF_HOSTS = ["tracked", "settled"];
const GE_APPLY_NAMES = [...GE_STEPS.map((step) => `ge-${step}-apply`), "ge-idle-apply"];
const GE_COMPUTE_NAMES = [...["memo", "effect"].flatMap((host) => GE_STEPS.map((step) => `ge-${step}-${host}`)), "ge-idle-memo"];
const GE_ACTION_NAMES = [...GE_STEPS.map((step) => `ge-${step}-action`), "ge-idle-action"];
/** A leaf host's contexts: a step each, and the idle twin (not for CLEANUP_IN_FORBIDDEN_SCOPE: a cleanup has no legal handler, NO_OWNER_CLEANUP). */
export const geLeafNames = (host, code = null) => [...GE_STEPS.map((step) => `ge-${step}-${host}`), ...(code === "CLEANUP_IN_FORBIDDEN_SCOPE" ? [] : [`ge-idle-${host}`])];
const GE_PATTERN = new RegExp(`^ge-(${[...GE_STEPS, "idle"].join("|")})-(apply|memo|effect|action|${GE_LEAF_HOSTS.join("|")})$`);
export const GE_CONTEXTS = Object.fromEntries([...GE_APPLY_NAMES, ...GE_COMPUTE_NAMES, ...GE_ACTION_NAMES, ...GE_LEAF_HOSTS.flatMap((host) => geLeafNames(host))].map((name) => [name, name.startsWith("ge-idle-") ? "legal" : "misuse"]));

/** ADR 0961 (PD): the parameter-initializer family. A host's callback calls a function whose parameter's default holds the site. */
export const PD_FORMS = ["value", "iife", "closure", "helper", "destructured", "ctor"];
export const PD_HOST_CODES = {
  action: ["FLUSH_IN_ACTION"],
  apply: ["FLUSH_IN_EFFECT_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK"],
  tracked: ["FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "CLEANUP_IN_FORBIDDEN_SCOPE"],
  settled: ["FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "CLEANUP_IN_FORBIDDEN_SCOPE"],
  await: ["UNTRACKED_READ_AFTER_AWAIT"],
};
export const PD_HOSTS = Object.keys(PD_HOST_CODES);
/**
 * The contexts of one host: a misuse form each, then the two legal twins. `onCleanup` has no legal handler (NO_OWNER_CLEANUP: a cleanup with no owner is a
 * defect of its own), so CLEANUP_IN_FORBIDDEN_SCOPE has the twin that passes the argument only.
 */
export const pdNames = (host, code = null) => [...PD_FORMS.map((form) => `pd-${form}-${host}`), `pd-passed-${host}`, ...(code === "CLEANUP_IN_FORBIDDEN_SCOPE" ? [] : [`pd-idle-${host}`])];
const PD_PATTERN = new RegExp(`^pd-(${[...PD_FORMS, "passed", "idle"].join("|")})-(${PD_HOSTS.join("|")})$`);
export const PD_CONTEXTS = Object.fromEntries(PD_HOSTS.flatMap((host) => pdNames(host)).map((name) => [name, /^pd-(passed|idle)-/.test(name) ? "legal" : "misuse"]));
/** The twin that passes the argument never runs its site by construction: a legal cell whose site does not run, as the dead-site contexts (misuse.mjs DEAD_CONTEXTS). */
export const PD_NEVER_RUN = PD_HOSTS.map((host) => `pd-passed-${host}`);

/** `{ [context]: "misuse" | "legal" }`: the side a context is on for the codes it is placed for. */
export const ENTRY_CONTEXTS = { ...ENTRY_ACTION_CONTEXTS, ...Object.fromEntries(TW_NAMES.map((name) => [name, twParts(name).order === "both" ? "legal" : "misuse"])), ...GE_CONTEXTS, ...PD_CONTEXTS };
export const ENTRY_CONTEXT_NAMES = Object.keys(ENTRY_CONTEXTS);

/** The contexts each code is placed for: the action family on FLUSH_IN_ACTION, a code's twice-entered shapes in its own tolerant layer, the generator-stepped and the parameter-initializer families. */
export const ENTRY_CODES = (() => {
  const placed = {};
  const place = (code, names) => { (placed[code] ??= []).push(...names); };
  place("FLUSH_IN_ACTION", Object.keys(ENTRY_ACTION_CONTEXTS));
  place("FLUSH_IN_ACTION", GE_ACTION_NAMES);
  for (const [code, layer] of Object.entries(TW_LAYERS)) place(code, TW_NAMES.filter((name) => twParts(name).layer === layer));
  for (const code of GE_APPLY_CODES) place(code, GE_APPLY_NAMES);
  for (const code of GE_COMPUTE_CODES) place(code, GE_COMPUTE_NAMES);
  for (const code of GE_LEAF_CODES) for (const host of GE_LEAF_HOSTS) place(code, geLeafNames(host, code));
  for (const [host, codes] of Object.entries(PD_HOST_CODES)) for (const code of codes) place(code, pdNames(host, code));
  return placed;
})();

/** The variants a code's cells of the twice-entered contexts do not take: under `runWithOwner(null, …)` an effect has no owner and reports NO_OWNER_EFFECT, a code of its own (PL_MEASURED `without`). */
export const ENTRY_NOT_VARIANTS = { PRIMITIVE_IN_FORBIDDEN_SCOPE: ["effect"] };

const generator = (name, stmts) => ({ ...ctl("gstep", stmts), name, mode: "first" });
/** The statement that enters `g` under a layer: `isPending(g)` or `runWithOwner(null, g)`. */
const enter = (layer, g) => (layer === "ispending" ? expr(prim("isPending", ref(g))) : expr(prim("owner", ref(g), { ownerArg: "null" })));
const enterText = (layer, g) => (layer === "ispending" ? `isPending(${g})` : `runWithOwner(null, ${g})`);
const solidOf = (layer) => (layer === "ispending" ? ["isPending"] : ["runWithOwner"]);

function actionContext(context, stmts, kit) {
  const a = kit.name("a");
  const b = kit.name("b");
  const stepped = (name) => button(b, [expr(domExpr(`void [...${name}()]`))]);
  const called = (name) => button(b, [expr(domExpr(`${name}()`))]);
  switch (context) {
    case "ae-helper": case "ae-helper-plain": {
      const run = kit.name("run");
      const plain = context.endsWith("-plain");
      return { body: [decl(run, domExpr(plain ? RUN_PLAIN : RUN_ACTION, undefined, plain ? undefined : ["action"])), decl(a, prim("action", fn(stmts), { via: ref(run) }))], view: [plain ? stepped(a) : called(a)] };
    }
    case "ae-helper-named": case "ae-helper-named-plain": {
      const run = kit.name("run");
      const g = kit.name("g");
      const plain = context.endsWith("-plain");
      return { body: [generator(g, stmts), decl(run, domExpr(plain ? RUN_PLAIN : RUN_ACTION, undefined, plain ? undefined : ["action"])), decl(a, domExpr(`${run}(${g})`))], view: [plain ? stepped(a) : called(a)] };
    }
    case "ae-package": case "ae-package-plain": {
      const plain = context.endsWith("-plain");
      return { body: [decl(a, prim("action", fn(stmts), { via: ref(ACTION_PACKAGE_CALLEE) }))], view: [plain ? stepped(a) : called(a)], packages: { [ACTION_PACKAGE]: actionPackageFiles(plain) }, importLines: [`import { ${ACTION_PACKAGE_CALLEE} } from "${ACTION_PACKAGE}";`] };
    }
    case "ae-delegate": {
      const g = kit.name("g");
      return { body: [generator(g, stmts), decl(a, prim("action", fn([expr(domExpr(`yield* ${g}()`))])))], view: [called(a)] };
    }
    case "ae-delegate-plain": {
      const g = kit.name("g");
      const outer = kit.name("o");
      return { body: [generator(g, stmts), expr(domExpr(`function* ${outer}() { yield* ${g}(); }`))], view: [stepped(outer)] };
    }
    default: return undefined;
  }
}

function twiceContext(context, stmts, kit) {
  const { layer, spelling, order, host, place } = twParts(context);
  const g = kit.name("g");
  const entries = (entry) => (order === "both" ? [entry(true), entry(true)] : order === "first" ? [entry(true), entry(false)] : [entry(false), entry(true)]);
  // The function entered twice, and the statements that enter it (`call`: the layer and a direct call; `flag`: a helper `make` that picks the entry by its argument).
  const declared = [decl(g, fn(stmts))];
  let entering;
  if (spelling === "call") {
    entering = entries((tolerant) => (tolerant ? enter(layer, g) : expr(call(ref(g)))));
  } else {
    const make = kit.name("make");
    const choose = `if (p0) { ${enterText(layer, g)}; } else { ${g}(); }`;
    declared.push(decl(make, fn([expr(domExpr(choose, undefined, solidOf(layer)))], { params: [bareParam("boolean", false)] })));
    entering = entries((tolerant) => expr(domExpr(`${make}(${tolerant})`)));
  }
  if (!place) return { body: [expr(prim(host, fn([...declared, ...entering])))], view: [] };
  // The entries are the body of a helper the host's callback calls, in the callback (`in`) or in the component's body (`out`).
  const both = kit.name("both");
  const helper = decl(both, fn(entering));
  const run = expr(call(ref(both)));
  return place === "in" ? { body: [expr(prim(host, fn([...declared, helper, run])))], view: [] } : { body: [...declared, helper, expr(prim(host, fn([run])))], view: [] };
}

/** ADR 0961 (F2, R1): the site in a generator the host's callback steps (`ge-<step>-<host>`), or discards (`ge-idle-<host>`). */
function steppedContext(context, stmts, kit) {
  const [, step, host] = GE_PATTERN.exec(context);
  const g = kit.name("g");
  const s = kit.name("s");
  // `anext`: an async generator, whose `.next()` runs the body up to its first await or yield at once.
  const declared = [step === "anext" ? { ...generator(g, stmts), async: true } : generator(g, stmts)];
  let steps;
  switch (step) {
    case "anext": steps = [expr(domExpr(`void ${g}().next()`))]; break;
    case "forof": steps = [expr(domExpr(`for (const ${s} of ${g}()) void ${s}`))]; break;
    case "spread": steps = [expr(domExpr(`void [...${g}()]`))]; break;
    case "next": { const it = kit.name("it"); steps = [decl(it, domExpr(`${g}()`)), expr(domExpr(`${it}.next()`))]; break; }
    case "yield": { const o = kit.name("o"); declared.push(expr(domExpr(`function* ${o}() { yield* ${g}(); }`))); steps = [expr(domExpr(`for (const ${s} of ${o}()) void ${s}`))]; break; }
    case "from": steps = [expr(domExpr(`void Array.from(${g}())`))]; break;
    case "direct": steps = [expr(domExpr(`${g}().next()`))]; break;
    case "set": steps = [expr(domExpr(`void new Set(${g}())`))]; break;
    case "stored": { const it = kit.name("it"); steps = [decl(it, domExpr(`${g}()`)), expr(domExpr(`for (const ${s} of ${it}) void ${s}`))]; break; }
    default: steps = [expr(domExpr(`${g}()`))];
  }
  // The legal twin steps the generator where the site is legal: a button's handler (the host only discards the iterator).
  let idle = [];
  if (step === "idle") {
    const h = kit.name("s");
    idle = [button(kit.name("b"), [expr(domExpr(`for (const ${h} of ${g}()) void ${h}`))])];
  }
  switch (host) {
    case "action": { const a = kit.name("a"); return { body: [...declared, decl(a, prim("action", fn(steps)))], view: [button(kit.name("b"), [expr(domExpr(`${a}()`))]), ...idle] }; }
    case "apply": return { body: [...declared, expr(prim("effect", fn([ret(num(1))]), { apply: fn(steps) }))], view: idle };
    case "effect": return { body: [...declared, expr(prim("effect", fn([...steps, ret(num(1))]), { apply: fn([]) }))], view: idle };
    case "tracked": case "settled": return { body: [...declared, expr(prim(host, fn(steps)))], view: idle };
    default: { const m = kit.name("m"); return { body: [...declared, decl(m, prim("memo", fn([...steps, ret(num(1))])))], view: [child(kit.name("v"), call(ref(m))), ...idle] }; }
  }
}

/** ADR 0961 (PD): the site in a parameter initializer of `h`, which the host's callback calls (`pd-<form>-<host>`), calls with an argument (`pd-passed-<host>`) or leaves to a handler (`pd-idle-<host>`). */
function parameterContext(context, stmts, kit) {
  const [, form, host] = PD_PATTERN.exec(context);
  // Names of their own: the helper layers write `h1` and `f1`, and a `const h1 = (p0 = h1(…))` would call itself.
  const h = kit.name("pdf");
  // A parameter with a default, annotated `unknown` (or the closure's type) so that a call can pass an argument whatever the default's type is.
  const withDefault = (def, type) => ({ ...param(def), type });
  const single = stmts.length === 1 && stmts[0].k === "expr" ? stmts[0].e : null;
  const declared = [];
  let helper;
  let constructed = null;
  switch (form) {
    case "value": helper = fn([ret(ref("p0"))], { params: [withDefault(single ?? iife(fn([...stmts, ret(num(1))])), "unknown")] }); break;
    case "destructured": helper = fn([ret(ref("p0"))], { params: [{ ...param(single ?? iife(fn([...stmts, ret(num(1))])), true), type: "unknown" }] }); break;
    case "ctor": constructed = contain("class-ctor-default", 8, [expr(single ?? iife(fn([...stmts, ret(num(1))])))], { use: "new" }); break;
    case "iife": helper = fn([ret(ref("p0"))], { params: [withDefault(iife(fn([...stmts, ret(num(1))])), "unknown")] }); break;
    case "closure": helper = fn([expr(call(ref("p0")))], { params: [withDefault(fn(stmts), "() => void")] }); break;
    case "helper": {
      const g = kit.name("pdg");
      declared.push(decl(g, fn(stmts)));
      helper = fn([ret(ref("p0"))], { params: [withDefault(call(ref(g)), "unknown")] });
      break;
    }
    // The twins: the default holds the site as `value` writes it.
    default: helper = fn([ret(ref("p0"))], { params: [withDefault(single ?? iife(fn([...stmts, ret(num(1))])), "unknown")] });
  }
  if (helper) declared.push(decl(h, helper));
  // What the host's callback does: calls `h()`, calls `h(1)` (the default is not evaluated), constructs the class, or nothing (a handler calls it).
  const idle = form === "idle";
  const entering = idle ? [] : constructed ? [constructed] : [expr(call(ref(h), form === "passed" ? [num(1)] : []))];
  const handler = idle ? [button(kit.name("b"), [expr(call(ref(h)))])] : [];
  switch (host) {
    case "action": { const a = kit.name("a"); return { body: [...declared, decl(a, prim("action", fn(entering)))], view: [button(kit.name("b"), [expr(domExpr(`${a}()`))]), ...handler] }; }
    case "apply": return { body: [...declared, expr(prim("effect", fn([ret(num(1))]), { apply: fn(entering) }))], view: handler };
    case "await": { const m = kit.name("m"); return { body: [...declared, decl(m, prim("memo", fn([awaitStmt(), ...entering, ret(num(1))], { async: true }))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: handler }; }
    default: return { body: [...declared, expr(prim(host, fn(entering)))], view: handler };
  }
}

/** The placement of `stmts` for an entry context (`{ body, view }`), or undefined for any other. */
export function entryContext(context, stmts, kit) {
  if (!ENTRY_CONTEXTS[context]) return undefined;
  if (PD_PATTERN.test(context)) return parameterContext(context, stmts, kit);
  if (GE_PATTERN.test(context)) return steppedContext(context, stmts, kit);
  return twParts(context) ? twiceContext(context, stmts, kit) : actionContext(context, stmts, kit);
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function entryPlacement() {
  return Object.fromEntries(Object.entries(ENTRY_CODES).map(([code, contexts]) => [code, {
    misuse: contexts.filter((context) => ENTRY_CONTEXTS[context] === "misuse"),
    legal: contexts.filter((context) => ENTRY_CONTEXTS[context] === "legal"),
  }]));
}

/** `{ [code]: { [context]: variants } }` for SPEC `recallVariantsNotIn`. */
export function entryNotVariants() {
  return Object.fromEntries(Object.entries(ENTRY_NOT_VARIANTS).map(([code, variants]) => [code, Object.fromEntries((ENTRY_CODES[code] ?? []).filter((context) => twParts(context)).map((context) => [context, variants]))]));
}
