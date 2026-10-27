// The construct tree a generated program is made of, and the rules that make a
// tree a valid program.
//
// A program is data, not text: every node is a plain `{ k: <kind>, ... }` object,
// so the shrinker can edit it by path, the printer renders it, and `validate`
// decides whether it is a type-correct Solid 2 file without running anything.
// Generation builds only valid trees (references are constructed in order, never
// repaired afterwards); `validate` exists for the shrinker, whose edits may break
// a reference, and as a test that the generator keeps its promise.
//
// Node kinds
//   expressions: num read write ref member obj field call iife iter fn param prim
//   statements:  expr decl return await
//   view items:  probe button child attr show for
//
// A `read` or `write` with `site: true` is a *site of interest*: the checker and
// the runtime are compared at its line. `witness: { selector, text }` says what
// the page shows when the site ran and landed (null when nothing shows it).

export const VERSION = 1;

/** Helper forms: a same-file function taking `cb: () => unknown`. */
export const HELPER_FORMS = ["call", "defaultCall", "returnClosure", "returnCb", "storeCb", "timerCb"];
/** `prim` names: the Solid primitive or host API a function expression is handed to. */
export const PRIMS = ["memo", "effect", "untrack", "root", "owner", "settled", "cleanup", "timer", "then"];

export const num = (n) => ({ k: "num", n });
export const read = (sig, extra = {}) => ({ k: "read", sig, ...extra });
export const write = (sig, value, extra = {}) => ({ k: "write", sig, value, ...extra });
export const ref = (name) => ({ k: "ref", name });
export const member = (obj, key) => ({ k: "member", obj, key });
export const obj = (fields) => ({ k: "obj", fields: fields.map(([key, e]) => ({ k: "field", key, e })) });
export const call = (callee, args = []) => ({ k: "call", callee, args });
/** `setSink(<expr>)`: writes an expression's number to a sink signal (never a site). */
export const set = (sig, e) => ({ k: "set", sig, e });
export const getOwner = () => ({ k: "getOwner" });
export const iife = (fn) => ({ k: "iife", fn });
export const iter = (method, fn) => ({ k: "iter", method, fn });
export const fn = (body, { params = [], form = "arrow", async = false } = {}) => ({ k: "fn", form, async, params, body });
export const param = (def, destructured = false) => ({ k: "param", destructured, default: def });
export const prim = (name, f, extra = {}) => ({ k: "prim", name, fn: f, ...extra });
export const expr = (e) => ({ k: "expr", e });
export const decl = (name, e) => ({ k: "decl", name, e });
export const ret = (e) => ({ k: "return", e });
export const awaitStmt = () => ({ k: "await" });

export const probe = (sig) => ({ k: "probe", sig });
export const button = (id, handler) => ({ k: "button", id, handler });
export const child = (id, e) => ({ k: "child", id, e });
export const attr = (id, e) => ({ k: "attr", id, e });
export const show = (when, children, fallback = []) => ({ k: "show", when, children, fallback });
export const forEach = (children) => ({ k: "for", children });

export const signal = (name, init, role = "src", scope = "component") => ({
  k: "signal", name, setter: `set${name[0].toUpperCase()}${name.slice(1)}`, init, role, scope,
});

// ---------------------------------------------------------------------------
// Generic traversal

const isNode = (value) => value !== null && typeof value === "object" && typeof value.k === "string";

/** The child-bearing properties of a node: [key, isList] pairs. */
export function slotsOf(node) {
  const slots = [];
  for (const [key, value] of Object.entries(node)) {
    if (key === "witness") continue;
    if (Array.isArray(value)) { if (value.length && value.every(isNode)) slots.push([key, true]); }
    else if (isNode(value)) slots.push([key, false]);
  }
  return slots;
}

/** Every node under (and including) `node`, depth first, with its path from the root. */
export function* walk(node, path = []) {
  yield { node, path };
  for (const [key, isList] of slotsOf(node)) {
    if (isList) for (let index = 0; index < node[key].length; index += 1) yield* walk(node[key][index], [...path, key, index]);
    else yield* walk(node[key], [...path, key]);
  }
}

export const nodeCount = (program) => { let count = 0; for (const _ of walk(program)) count += 1; return count; };

export function getAt(root, path) {
  return path.reduce((value, key) => value[key], root);
}

/** A deep copy, safe to edit. */
export const clone = (value) => structuredClone(value);

// ---------------------------------------------------------------------------
// Types

const T = {
  num: { t: "num" }, void: { t: "void" }, unknown: { t: "unknown" },
  fn: (r, needs = 0) => ({ t: "fn", ret: r, needs }),
  acc: (of) => ({ t: "acc", of }),
  obj: (fields) => ({ t: "obj", fields }),
};
const same = (a, b) => a.t === b.t;

export function tsType(type) {
  switch (type.t) {
    case "num": return "number";
    case "void": return "void";
    default: return "unknown";
  }
}

const helperType = (form) => ({ t: "helper", form });
function helperResult(form, supplied) {
  switch (form) {
    case "call": case "timerCb": return T.num;
    case "returnClosure": return T.fn(T.unknown);
    case "returnCb": return T.fn(T.unknown);
    case "storeCb": return T.obj({ cb: T.fn(T.unknown) });
    default: return T.unknown;
  }
}

/** Returns the signal table of a program: name -> signal. */
export function signalTable(program) {
  const table = new Map();
  for (const sig of program.moduleSignals ?? []) table.set(sig.name, sig);
  for (const component of program.components) for (const sig of component.signals) table.set(sig.name, sig);
  return table;
}

// ---------------------------------------------------------------------------
// Validation: a small type checker for the node language. It rejects exactly the
// compositions that would not be valid, type-correct Solid 2 (`tsrs` is the
// authority; this is the fast pre-filter, the shrinker's guard and the
// generator's contract).

/**
 * What a function handed to a primitive may return, as the published typings
 * say: a compute returns a value (a number here), an effect's apply and
 * onSettled/onCleanup callbacks return `void | (() => void)`, which accepts
 * `undefined` or a function but not a number. `free` places no constraint.
 */
const RETURNS = { memo: "any", effect: "any", applyFn: "voidOrFn", settled: "voidOrFn", cleanup: "voidOrFn" };

export function validate(program, { maxNodes = 400 } = {}) {
  const errors = [];
  const fail = (message) => { if (!errors.includes(message)) errors.push(message); };
  if (program.version !== VERSION) fail(`version ${program.version}`);
  if (nodeCount(program) > maxNodes) fail("too many nodes");
  if (!program.components.length) fail("no component");
  const helpers = new Map((program.helpers ?? []).map((helper) => [helper.name, helper]));
  const seenSignals = new Set();
  for (const sig of [...(program.moduleSignals ?? []), ...program.components.flatMap((component) => component.signals)]) {
    if (seenSignals.has(sig.name)) fail(`duplicate signal ${sig.name}`);
    seenSignals.add(sig.name);
  }
  const componentNames = new Set();
  for (const component of program.components) {
    if (componentNames.has(component.name)) fail(`duplicate component ${component.name}`);
    componentNames.add(component.name);
    if (!/^[A-Z]/.test(component.name)) fail(`component ${component.name} is not capitalized`);
    checkComponent(component);
  }
  return errors;

  function checkComponent(component) {
    const where = component.name;
    const own = new Set([...(program.moduleSignals ?? []).map((s) => s.name), ...component.signals.map((s) => s.name)]);
    const buttons = new Set();
    const scopeOf = (parent, extra = {}) => ({ vars: new Map(), parent, async: parent?.async ?? false, ...extra });
    const lookup = (scope, name) => {
      for (let s = scope; s; s = s.parent) if (s.vars.has(name)) return s.vars.get(name);
      return undefined;
    };
    const sigCheck = (name, use) => { if (!own.has(name)) fail(`${where}: ${use} of signal ${name} that is not in scope`); };
    const root = scopeOf(null);
    checkBlock(component.body, root, "free");
    for (const item of component.view) checkView(item, root);

    function checkView(item, scope) {
      switch (item.k) {
        case "probe": sigCheck(item.sig, "probe"); break;
        case "button": {
          if (buttons.has(item.id)) fail(`${where}: duplicate button ${item.id}`);
          buttons.add(item.id);
          checkBlock(item.handler, scopeOf(scope), "free");
          break;
        }
        case "child": case "attr": {
          const type = typeOf(item.e, scope);
          if (type && type.t !== "num") fail(`${where}: a JSX ${item.k} must be a number, not ${type.t}`);
          break;
        }
        case "show": {
          const type = typeOf(item.when, scope);
          if (type && type.t !== "num") fail(`${where}: Show when must be a number`);
          for (const c of [...item.children, ...item.fallback]) checkView(c, scope);
          break;
        }
        case "for": for (const c of item.children) checkView(c, scope); break;
        default: fail(`${where}: unknown view kind ${item.k}`);
      }
    }

    function checkBlock(stmts, scope, returns) {
      let result = T.void;
      stmts.forEach((stmt, index) => {
        switch (stmt.k) {
          case "expr": typeOf(stmt.e, scope); break;
          case "decl": {
            if (scope.vars.has(stmt.name)) fail(`${where}: ${stmt.name} declared twice`);
            const type = typeOf(stmt.e, scope);
            if (type) scope.vars.set(stmt.name, type);
            if (stmt.e.k !== "fn" && stmt.e.form) fail(`${where}: only a function has a form`);
            break;
          }
          case "return": {
            if (index !== stmts.length - 1) fail(`${where}: return is not last`);
            const type = typeOf(stmt.e, scope);
            if (type) result = type;
            break;
          }
          case "await":
            if (!scope.async) fail(`${where}: await outside an async function`);
            break;
          default: fail(`${where}: unknown statement ${stmt.k}`);
        }
      });
      if (returns === "voidOrFn" && result.t !== "fn" && result.t !== "void") fail(`${where}: this callback may return only a function`);
      if (returns === "any" && (result.t === "void")) fail(`${where}: a compute must return a value`);
      if (returns === "num" && result.t !== "num") fail(`${where}: an effect compute must return a number`);
      return result;
    }

    function fnTypeChecked(f, scope, returns, vars = []) {
      if (f.k === "ref" && returns === "free") {
        const type = typeOf(f, scope);
        if (type && type.t === "fn") return type;
        fail(`${where}: expected a function`);
        return T.fn(T.unknown);
      }
      if (f.k !== "fn") { fail(`${where}: expected a function literal`); return T.fn(T.unknown); }
      if (f.async && returns === "voidOrFn") fail(`${where}: an async callback is not (() => void) | void`);
      const inner = scopeOf(scope, { async: f.async });
      for (const [name, type] of vars) inner.vars.set(name, type);
      f.params.forEach((p, index) => {
        const type = typeOf(p.default, inner);
        if (type && (type.t === "fn" || type.t === "acc")) fail(`${where}: a function-typed default is not generated`);
        // The parameter is in scope for later defaults and the body (`p0`, `p1`, ...). Its type is
        // its printed annotation (exprTsType), not the default's inferred literal type.
        const annotation = paramTsType(p);
        inner.vars.set(`p${index}`, annotation === "number" ? T.num : annotation === "void" ? T.void : T.unknown);
      });
      const r = checkBlock(f.body, inner, f.async ? "free" : returns);
      if (f.async && returns === "num" && r.t !== "num") fail(`${where}: an effect compute must return a number`);
      return T.fn(f.async ? T.unknown : r, 0);
    }

    function typeOf(e, scope) {
      switch (e.k) {
        case "num": return T.num;
        case "read": sigCheck(e.sig, "read"); return T.num;
        case "write": sigCheck(e.sig, "write"); return T.num;
        case "ref": {
          const type = lookup(scope, e.name);
          if (!type) { fail(`${where}: ${e.name} is not declared before use`); return null; }
          return type;
        }
        case "member": {
          const base = typeOf(e.obj, scope);
          if (!base) return null;
          if (base.t !== "obj" || !base.fields[e.key]) { fail(`${where}: no member ${e.key}`); return null; }
          return base.fields[e.key];
        }
        case "obj": return T.obj(Object.fromEntries(e.fields.map((f) => [f.key, typeOf(f.e, scope) ?? T.unknown])));
        case "fn": return fnTypeChecked(e, scope, "free");
        case "call": {
          const isHelper = e.callee.k === "ref" && helpers.has(e.callee.name) && lookup(scope, e.callee.name) === undefined;
          const callee = isHelper ? helperType(helpers.get(e.callee.name).form) : typeOf(e.callee, scope);
          const args = e.args.map((a) => typeOf(a, scope));
          if (!callee) return null;
          if (callee.t === "helper") {
            if (args.length < 1 || !args[0] || args[0].t !== "fn") { fail(`${where}: a helper takes a function`); return null; }
            if (args.length > 1 && callee.form !== "defaultCall") fail(`${where}: only defaultCall takes a second argument`);
            return helperResult(callee.form, args.length > 1);
          }
          if (callee.t === "acc") return callee.of;
          if (callee.t === "fn") return callee.ret;
          fail(`${where}: calling a ${callee.t}`);
          return null;
        }
        case "getOwner": return { t: "owner" };
        case "set": {
          sigCheck(e.sig, "set");
          const type = typeOf(e.e, scope);
          if (type && type.t !== "num") fail(`${where}: a sink takes a number, not ${type.t}`);
          return T.num;
        }
        case "iife": return typeOf(e.fn, scope)?.ret ?? null;
        case "iter": {
          const f = typeOf(e.fn, scope);
          if (!f) return null;
          return e.method === "map" ? f.ret : T.void;
        }
        case "prim": return primType(e, scope);
        default: fail(`${where}: unknown expression ${e.k}`); return null;
      }
    }

    function primType(e, scope) {
      switch (e.name) {
        case "memo": return T.acc(fnTypeChecked(e.fn, scope, RETURNS.memo).ret);
        case "effect": {
          const compute = fnTypeChecked(e.fn, scope, RETURNS.effect);
          if (!e.apply) fail(`${where}: effect without an apply`);
          else fnTypeChecked(e.apply, scope, RETURNS.applyFn, [["v", compute.ret]]);
          return T.void;
        }
        case "untrack": case "root": return fnTypeChecked(e.fn, scope, "free").ret;
        case "owner":
          if (e.ownerArg === "owner" && !lookup(scope, "owner")) fail(`${where}: owner is not captured`);
          return fnTypeChecked(e.fn, scope, "free").ret;
        case "settled": fnTypeChecked(e.fn, scope, RETURNS.settled); return T.void;
        case "cleanup": fnTypeChecked(e.fn, scope, RETURNS.cleanup); return T.void;
        case "timer": case "then": fnTypeChecked(e.fn, scope, "free"); return T.void;
        default: fail(`${where}: unknown prim ${e.name}`); return null;
      }
    }
  }
}

export const isValid = (program) => validate(program).length === 0;

/** A parameter's printed type annotation. A destructured `void` field is `unknown` (TypeScript rejects a void default there). */
export function paramTsType(p) {
  const type = exprTsType(p.default);
  return p.destructured && type === "void" ? "unknown" : type;
}

/** The annotation of an expression a default holds, from its syntax. */
export function exprTsType(e) {
  if (e.k === "set") return "number";
  switch (e.k) {
    case "num": case "read": case "write": return "number";
    case "iter": return e.method === "map" ? "unknown" : "void";
    default: return "unknown";
  }
}

export { T, same };

export const helper = (name, form) => ({ k: "helper", name, form });
export const component = (name, signals, body, view) => ({ k: "component", name, signals, body, view });
export const program = ({ seed = 0, helpers = [], moduleSignals = [], components }) => ({
  k: "program", version: VERSION, seed, helpers, moduleSignals, components,
});
