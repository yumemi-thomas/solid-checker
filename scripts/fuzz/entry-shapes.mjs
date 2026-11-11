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
// Recall contexts only, base shapes (misuse.mjs BASE_ONLY_CONTEXTS): the question is how the site is entered, not the spelling of the site. The census does not cross them
// with other codes (matrix.mjs CENSUS_SKIPS): they are the cells of the codes that name them, as the `pl-*` ones.
import { bareParam, button, call, ctl, decl, domExpr, expr, fn, prim, ref } from "./model.mjs";

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

/** `{ [context]: "misuse" | "legal" }`: the side a context is on for the codes it is placed for. */
export const ENTRY_CONTEXTS = { ...ENTRY_ACTION_CONTEXTS, ...Object.fromEntries(TW_NAMES.map((name) => [name, twParts(name).order === "both" ? "legal" : "misuse"])) };
export const ENTRY_CONTEXT_NAMES = Object.keys(ENTRY_CONTEXTS);

/** The contexts each code is placed for: the action family on FLUSH_IN_ACTION, a code's twice-entered shapes in its own tolerant layer. */
export const ENTRY_CODES = {
  FLUSH_IN_ACTION: Object.keys(ENTRY_ACTION_CONTEXTS),
  ...Object.fromEntries(Object.entries(TW_LAYERS).map(([code, layer]) => [code, TW_NAMES.filter((name) => twParts(name).layer === layer)])),
};

/** The variants a code's cells of these contexts do not take: under `runWithOwner(null, …)` an effect has no owner and reports NO_OWNER_EFFECT, a code of its own (PL_MEASURED `without`). */
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

/** The placement of `stmts` for an entry context (`{ body, view }`), or undefined for any other. */
export function entryContext(context, stmts, kit) {
  if (!ENTRY_CONTEXTS[context]) return undefined;
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
  return Object.fromEntries(Object.entries(ENTRY_NOT_VARIANTS).map(([code, variants]) => [code, Object.fromEntries((ENTRY_CODES[code] ?? []).map((context) => [context, variants]))]));
}
