// Seeded generator: seed -> a small, type-correct Solid 2 program (a construct tree).
//
// Each component carries one *subject*: a payload (a write to a probe signal, or a
// read of a source signal) wrapped in 0-3 layers (closures, parameter defaults,
// IIFEs, forEach/map, same-file helpers) and placed in a *host* scope (a memo or
// effect compute, a component body, a JSX expression, an apply, onSettled, a timer,
// a promise callback, an async function, a handler, untrack/createRoot/runWithOwner)
// with a *disposition* (called there, returned, stored, ignored, called later from a
// click). The families of ADR 0452 (a closure a tracked scope builds but does not
// call; defaults with an IIFE or forEach inside; helpers returning closures) are
// points in this space, weighted up by profile; the rest of the space is the search.
//
// Every program is a pure function of its seed and `GENERATOR_VERSION`. Structure is
// constructed in order (a name is declared before it is used); the few typing corner
// cases a construction can still hit (a `forEach` result routed into a JSX child) are
// removed by validating the finished component and drawing again, up to a bound.
import { createRng } from "./rng.mjs";
import { buildDisposal } from "./disposal.mjs";
import { chooseLayersPackages, ensurePkgHelper, firePlacement, packagePayloadKind, PACKAGE_HOSTS } from "./packages.mjs";
import {
  validate, signal, num, read, write, ref, member, obj, call, iife, iter, fn, param, prim, expr, decl, ret,
  awaitStmt, probe, button, child, attr, show, forEach, set, exprTsType, getOwner, helper, component, program as makeProgram,
  own, isSite, OWN_OPS,
} from "./model.mjs";

export const GENERATOR_VERSION = 2;

const PROFILES = [
  // [weight, name]
  [3, "closure-escape"], [3, "default-param"], [2, "helper-closure"], [2, "in-place"], [1, "async"], [2, "mixed"], [2, "body-call"],
];

/**
 * Owner profiles (not drawn at random: ask for them with `--profile`, so the seeds of the other profiles are unchanged).
 * The payload is an owner-requiring call (`onCleanup`, `createEffect`, `onSettled`, `useContext`) in any host: the runtime
 * warns or throws when no owner is current (NO_OWNER_*, SETTLED_CLEANUP_UNOWNED), or when the host forbids the call
 * (CLEANUP_IN_FORBIDDEN_SCOPE). A site shows nothing on the page; the hit ledger says it ran.
 */
export const OWNER_PROFILES = { "owner-cleanup": "cleanup", "owner-effect": "effect", "owner-settled": "settled", "owner-context": "context", owner: null };

const HOSTS = ["memo", "effect", "untrack", "root", "owner", "body", "apply", "settled", "cleanup", "timer", "then", "handler", "jsx", "async"];
/** Hosts a control subject uses: a leaf scope where a write or read is legal. */
const LEGAL_HOSTS = ["apply", "settled", "handler", "timer", "then"];

const hostWeights = {
  "closure-escape": { memo: 5, effect: 3, untrack: 2, root: 2, owner: 2, body: 2, apply: 1, settled: 1, cleanup: 1, timer: 1, then: 1, handler: 1, jsx: 1, async: 0 },
  "default-param": { memo: 5, effect: 2, untrack: 1, root: 1, owner: 1, body: 2, apply: 1, settled: 1, cleanup: 0, timer: 1, then: 1, handler: 1, jsx: 1, async: 0 },
  "helper-closure": { memo: 6, effect: 3, untrack: 1, root: 1, owner: 1, body: 2, apply: 1, settled: 1, cleanup: 0, timer: 0, then: 0, handler: 1, jsx: 1, async: 0 },
  "in-place": { memo: 4, effect: 3, untrack: 2, root: 2, owner: 2, body: 2, apply: 3, settled: 2, cleanup: 1, timer: 1, then: 1, handler: 2, jsx: 2, async: 0 },
  async: { memo: 1, effect: 2, untrack: 0, root: 0, owner: 0, body: 1, apply: 1, settled: 1, cleanup: 0, timer: 2, then: 3, handler: 1, jsx: 0, async: 5 },
  mixed: Object.fromEntries(HOSTS.map((host) => [host, 1])),
  // A memo that builds a closure the component body then calls (`m()()`): the read is untracked there (rc.17 warns).
  "body-call": { memo: 8, effect: 1, untrack: 0, root: 1, owner: 0, body: 1, apply: 0, settled: 0, cleanup: 0, timer: 0, then: 0, handler: 0, jsx: 1, async: 0 },
  // Generated dependency packages (packages.mjs): the helper layers are package exports; asked for with `--profile packages`.
  packages: PACKAGE_HOSTS,
  // Owner payloads: ownerless hosts (handler, timer, promise, runWithOwner(null), untrack) and owned ones (body, memo, effect, root) both.
  ...Object.fromEntries(Object.keys(OWNER_PROFILES).map((name) => [name, { memo: 3, effect: 3, untrack: 3, root: 3, owner: 4, body: 3, apply: 3, settled: 3, cleanup: 2, timer: 3, then: 3, handler: 3, jsx: 0, async: 2 }])),
};

const layerWeights = (profile) => ({
  "closure-escape": { closure: 6, default: 1, iife: 1, iter: 1, helper: 1 },
  "default-param": { closure: 1, default: 6, iife: 3, iter: 3, helper: 1 },
  "helper-closure": { closure: 2, default: 1, iife: 1, iter: 1, helper: 6 },
  "in-place": { closure: 1, default: 0, iife: 4, iter: 4, helper: 1 },
  async: { closure: 3, default: 0, iife: 1, iter: 1, helper: 1 },
  mixed: { closure: 3, default: 3, iife: 2, iter: 2, helper: 2 },
  "body-call": { closure: 6, default: 1, iife: 1, iter: 1, helper: 2 },
  ...Object.fromEntries(Object.keys(OWNER_PROFILES).map((name) => [name, { closure: 4, default: 1, iife: 2, iter: 1, helper: 2 }])),
})[profile];

export function generateProgram(seed, options = {}) {
  const rng = createRng(seed);
  const profile = options.profile ?? rng.weighted(PROFILES);
  // One component per page: a thrown dev error halts reactivity for every root on the page, so a second
  // component would hide or fake the first one's behaviour. More than one is for experiments only.
  const count = options.components ?? 1;
  const ctx = { rng, profile, plan: options.disposalPlan ?? options.packagePlan, counters: {}, helpers: new Map(), moduleSignals: [] };
  const components = [];
  for (let index = 0; index < count; index += 1) {
    const control = index > 0 && rng.chance(0.5);
    components.push(buildValidComponent(ctx, index, control));
  }
  return makeProgram({ seed, helpers: [...ctx.helpers.values()], moduleSignals: ctx.moduleSignals, components });
}

function buildValidComponent(ctx, index, control) {
  const attempts = 12;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const snapshot = { counters: { ...ctx.counters }, helpers: new Map(ctx.helpers), moduleSignals: [...ctx.moduleSignals] };
    const candidate = buildComponent(ctx, index, control);
    const trial = makeProgram({ seed: 0, helpers: [...ctx.helpers.values()], moduleSignals: ctx.moduleSignals, components: [candidate] });
    if (validate(trial).length === 0) return candidate;
    // A drawn component that failed validation consumed names; restore them so a retry is stable.
    ctx.counters = snapshot.counters; ctx.helpers = snapshot.helpers; ctx.moduleSignals = snapshot.moduleSignals;
  }
  return fallbackComponent(ctx, index);
}

/** The smallest valid component: a write in a click handler. Reached only when 12 draws fail. */
function fallbackComponent(ctx, index) {
  const w = name(ctx, "w");
  const value = 10 + 7 * counter(ctx, "value");
  return component(`C${index + 1}`, [signal(w, 0, "probe")], [], [
    probe(w), button(name(ctx, "b"), [expr(write(w, value, { site: true, witness: probeWitness(w, value) }))]),
  ]);
}

// ---------------------------------------------------------------------------

function counter(ctx, key) { ctx.counters[key] = (ctx.counters[key] ?? 0) + 1; return ctx.counters[key]; }
function name(ctx, prefix) { return `${prefix}${counter(ctx, prefix)}`; }
const probeWitness = (sig, value) => ({ selector: `[data-probe="${sig}"]`, text: String(value) });
const viewWitness = (id, value) => ({ selector: `[data-view="${id}"]`, text: String(value) });

function ensureHelper(ctx, form) {
  for (const existing of ctx.helpers.values()) if (existing.form === form) return existing.name;
  const added = helper(`h${ctx.helpers.size + 1}`, form);
  ctx.helpers.set(added.name, added);
  return added.name;
}

function buildComponent(ctx, index, control) {
  const { rng, profile } = ctx;
  if (profile === "disposal") {
    // Writes that run while an owner is disposed (disposal.mjs). Asked for with `--profile disposal`, never drawn at random.
    const built = buildDisposal({ rng, name: (prefix) => name(ctx, prefix), counter: (key) => counter(ctx, key), ensureHelper: (form) => ensureHelper(ctx, form) }, ctx.plan);
    return { ...component(`C${index + 1}`, built.sigs, built.body, built.view), shape: built.shape };
  }
  const sigs = [];
  const view = [];
  const body = [];
  const needsValue = rng.chance(0.4);
  const host = control ? rng.pick(LEGAL_HOSTS) : ctx.plan?.host ?? rng.weighted(Object.entries(hostWeights[profile]).map(([host, weight]) => [weight, host]));
  const mkSig = (prefix, init, role) => { const s = signal(name(ctx, prefix), init, role); sigs.push(s); return s; };

  // The payload and where its effect is shown.
  ctx.pendingFires = [];
  const pkgLayers = profile === "packages" ? ctx.plan?.layers ?? chooseLayersPackages(ctx) : null;
  const pkgKind = pkgLayers ? ctx.plan?.kind ?? packagePayloadKind(rng, pkgLayers) : null;
  const reads = pkgKind ? pkgKind === "read" : needsValue;
  let payload; let route = { needsValue: reads };
  if (profile in OWNER_PROFILES || pkgKind === "own") {
    payload = own(OWNER_PROFILES[profile] ?? rng.pick(OWN_OPS), { site: true, witness: null });
    route = { needsValue: false };
  } else if (reads) {
    const init = 1000 + 7 * counter(ctx, "value");
    const src = mkSig("r", init, "src");
    const sink = mkSig("k", 0, "sink");
    payload = read(src.name, { site: true, witness: probeWitness(sink.name, init) });
    route = { needsValue: reads, sink: sink.name, init, setSink: true };
    view.push(probe(sink.name));
  } else {
    const value = 10 + 7 * counter(ctx, "value");
    const target = mkSig("w", 0, "probe");
    payload = write(target.name, value, { site: true, witness: probeWitness(target.name, value) });
    view.push(probe(target.name));
  }

  const pre = [];
  const layers = pkgLayers ?? chooseLayers(ctx, host);
  let cur = { kind: "value", e: payload, key: null, dflt: null };
  for (const layer of layers) cur = applyLayer(ctx, layer, cur, pre);

  const disp = ctx.plan?.disp ?? chooseDisposition(ctx, host, cur);
  const placed = place(ctx, host, cur, pre, disp, route);
  body.push(...placed.body);
  view.push(...placed.view);
  if (placed.signals) sigs.push(...placed.signals);
  // A package `register` keeps the callback; its `fire` export calls it later (from a click, the body, or a timer).
  for (const fire of ctx.pendingFires) firePlacement(ctx, fire, { body, view, name: (prefix) => name(ctx, prefix) });

  // Legal code beside the subject: a checker violation here is a false positive by construction.
  if (rng.chance(0.5) && !ctx.plan?.plain) addDecoy(ctx, sigs, body, view);

  // JSX positions: a child expression may sit under Show or inside a For row.
  for (let i = 0; i < view.length; i += 1) {
    if (view[i].k !== "child") continue;
    if (rng.chance(0.15)) view[i] = show(num(1), [view[i]]);
    else if (rng.chance(0.08)) view[i] = forEach([view[i]]);
  }

  // A leaf subject may be wrapped in one more scope: an outer host whose body the subject becomes.
  const wrappable = placed.wrappable && rng.chance(0.3) && !ctx.plan?.plain;
  const finalBody = wrappable ? wrapOuter(ctx, body, view) : body;
  return component(`C${index + 1}`, sigs, finalBody, view);
}

function chooseLayers(ctx, host) {
  const { rng, profile } = ctx;
  const weights = layerWeights(profile);
  const depth = rng.weighted(profile === "in-place" ? [[2, 0], [4, 1], [3, 2]] : [[1, 0], [3, 1], [4, 2], [2, 3]]);
  const layers = [];
  for (let i = 0; i < depth; i += 1) {
    const kind = rng.weighted(Object.entries(weights).map(([k, w]) => [w, k]));
    switch (kind) {
      case "closure": layers.push({ k: "closure", named: rng.chance(0.6), form: rng.chance(0.2) ? "function" : "arrow", async: host === "async" && rng.chance(0.5) }); break;
      case "default": layers.push({ k: "default", named: rng.chance(0.6), destructured: rng.chance(0.35) }); break;
      case "iife": layers.push({ k: "iife" }); break;
      case "iter": layers.push({ k: "iter", method: rng.pick(["forEach", "map"]) }); break;
      default: layers.push({ k: "helper", form: rng.weighted([[3, "call"], [2, "defaultCall"], [4, "returnClosure"], [2, "returnCb"], [2, "storeCb"], [1, "timerCb"]]), supply: rng.chance(0.25) });
    }
  }
  return layers;
}

const invoke0 = (cur) => (cur.kind === "obj" ? call(member(cur.e, cur.key)) : call(cur.e));
const valueOf = (cur) => (cur.kind === "value" ? cur.e : invoke0(cur));
const invoke = (cur, supply) => {
  const args = supply && cur.dflt === "plain" ? [num(5)] : supply && cur.dflt === "destructured" ? [obj([["p0", num(5)]])] : [];
  // (a number is only a valid argument where the parameter's annotation takes one; see applyLayer)
  return cur.kind === "obj" ? call(member(cur.e, cur.key), args) : call(cur.e, args);
};

function applyLayer(ctx, layer, cur, pre) {
  const { rng } = ctx;
  const named = (f, extra = {}) => {
    if (layer.named === false) return { kind: "fn", e: f, key: null, dflt: null, ...extra };
    const id = name(ctx, "f");
    pre.push(decl(id, f));
    return { kind: "fn", e: ref(id), key: null, dflt: null, ...extra };
  };
  switch (layer.k) {
    case "closure": {
      const body = [ret(valueOf(cur))];
      if (layer.async) body.unshift(awaitStmt());
      const f = fn(body, { form: layer.form, async: Boolean(layer.async) });
      // A function declaration is a statement; an anonymous one is only an expression.
      return layer.form === "function" ? named(f) : named(f);
    }
    case "default": {
      const def = valueOf(cur);
      const f = fn([ret(ref("p0"))], { params: [param(def, layer.destructured)] });
      // A number is a valid argument only where the annotation takes one (number or unknown, not void).
      const takesNumber = layer.destructured || exprTsType(def) !== "void";
      return named(f, { dflt: takesNumber ? (layer.destructured ? "destructured" : "plain") : null });
    }
    case "iife": return { kind: "value", e: iife(fn([ret(valueOf(cur))])), key: null, dflt: null };
    case "iter": {
      const value = valueOf(cur);
      const f = layer.method === "map" ? fn([ret(value)]) : fn([expr(value)]);
      return { kind: "value", e: iter(layer.method, f), key: null, dflt: null };
    }
    default: {
      const fnExpr = cur.kind === "fn" ? cur.e : fn([ret(valueOf(cur))]);
      if (layer.k === "pkg") {
        const h = ensurePkgHelper(ctx, layer.pkg);
        const c = call(ref(h), [fnExpr]);
        switch (layer.pkg.behaviour) {
          case "returnCb": return { kind: "fn", e: c, key: null, dflt: cur.kind === "fn" ? cur.dflt : null };
          case "returnWrap": return { kind: "fn", e: c, key: null, dflt: null };
          case "storeObj": return { kind: "obj", e: c, key: "cb", dflt: null };
          case "register": ctx.pendingFires.push(ctx.fireOf.get(h)); return { kind: "value", e: c, key: null, dflt: null };
          default: return { kind: "value", e: c, key: null, dflt: null };
        }
      }
      const h = ensureHelper(ctx, layer.form);
      const supply = layer.form === "defaultCall" && layer.supply;
      const c = call(ref(h), [fnExpr, ...(supply ? [num(5)] : [])]);
      switch (layer.form) {
        case "returnClosure": return { kind: "fn", e: c, key: null, dflt: "plain" };
        case "returnCb": return { kind: "fn", e: c, key: null, dflt: cur.kind === "fn" ? cur.dflt : null };
        case "storeCb": return { kind: "obj", e: c, key: "cb", dflt: null };
        default: void rng; return { kind: "value", e: c, key: null, dflt: null };
      }
    }
  }
}

/** Where the subject's function goes once built. Value subjects just run. */
function chooseDisposition(ctx, host, cur) {
  const { rng } = ctx;
  if (cur.kind === "value") return "run";
  const escaping = ["memo", "effect", "untrack", "root", "owner"].includes(host);
  const returning = ["settled", "cleanup", "apply"].includes(host) && cur.kind === "fn";
  return rng.weighted([
    [3, "call"],
    [escaping ? 5 : 0, "return"],
    [escaping && cur.kind === "fn" ? 3 : 0, "store"],
    [returning ? 2 : 0, "return"],
    [host === "body" ? 4 : 0, "handler"],
    [1, "ignore"],
  ]);
}

const supplyChoice = (ctx, cur) => (cur.dflt ? ctx.rng.chance(0.2) : false);

/** Statement for a value in a leaf scope: a sink write when the value is wanted. */
function sinkStmt(route, valueExpr) {
  return route.needsValue ? expr(set(route.sink, valueExpr)) : expr(valueExpr);
}

function place(ctx, host, cur0, pre, disp, route) {
  // A function built and left alone must still be written down: bind an anonymous one to a name.
  let cur = cur0;
  if (disp === "ignore" && cur.kind !== "value" && cur.e.k !== "ref") {
    const id = name(ctx, "f");
    pre.push(decl(id, cur.e));
    cur = { ...cur, e: ref(id) };
  }
  const supply = supplyChoice(ctx, cur);
  const out = { body: [], view: [], wrappable: false };
  const viewId = () => name(ctx, "v");
  const ranValue = () => (cur.kind === "value" ? cur.e : invoke(cur, supply));
  const withWitness = (id) => {
    // The page text that shows a read ran, set on the payload by walking down to it.
    if (route.needsValue) setReadWitness(cur, pre, viewWitness(id, route.init));
  };
  const handlerFor = (access, id) => {
    const bid = name(ctx, "b");
    out.view.push(button(bid, [sinkStmt(route, invoke(access, supply))]));
    return bid;
  };
  const escape = (c, value) => (disp === "store" ? { kind: "obj", e: value, key: "f", dflt: c.dflt } : { ...c, e: value });

  switch (host) {
    case "memo": {
      const api = name(ctx, "m");
      const inner = [...pre];
      if (disp === "return") inner.push(ret(cur.e));
      else if (disp === "store") inner.push(ret(obj([["f", cur.e]])));
      else if (disp === "ignore") inner.push(ret(num(1)));
      else if (route.needsValue) inner.push(ret(ranValue()));
      else inner.push(expr(ranValue()), ret(num(1)));
      out.body.push(decl(api, prim("memo", fn(inner))));
      if ((disp === "return" || disp === "store") && ctx.profile === "body-call" && ctx.rng.chance(0.8)) {
        // The component body calls what the memo returned.
        const callExpr = invoke(escape(cur, call(ref(api))), supply);
        if (route.needsValue) {
          const id = name(ctx, "n");
          out.body.push(decl(id, callExpr));
          const vid = viewId();
          out.view.push(child(vid, ref(id)));
          withWitness(vid);
        } else out.body.push(expr(callExpr));
      } else if (disp === "return" || disp === "store") handlerFor(escape(cur, call(ref(api))), null);
      else { const id = viewId(); out.view.push(child(id, call(ref(api)))); withWitness(id); }
      return out;
    }
    case "effect": {
      const compute = [...pre];
      let applyStmts = [];
      if (disp === "return") compute.push(ret(cur.e));
      else if (disp === "store") compute.push(ret(obj([["f", cur.e]])));
      else if (disp === "ignore") compute.push(ret(num(1)));
      else if (route.needsValue) compute.push(ret(ranValue()));
      else compute.push(expr(ranValue()), ret(num(1)));
      if (disp === "return" || disp === "store") applyStmts = [sinkStmt(route, invoke(escape(cur, ref("v")), supply))];
      else if (route.needsValue && disp !== "ignore") applyStmts = [expr(set(route.sink, ref("v")))];
      out.body.push(expr(prim("effect", fn(compute), { apply: fn(applyStmts) })));
      return out;
    }
    case "untrack": case "root": case "owner": {
      const ownerArg = host === "owner" && ctx.rng.chance(0.3) ? "owner" : "null";
      const extra = host === "owner" ? { ownerArg } : {};
      if (ownerArg === "owner") out.body.push(decl("owner", getOwner()));
      const inner = [...pre];
      if (disp === "return") inner.push(ret(cur.e));
      else if (disp === "store") inner.push(ret(obj([["f", cur.e]])));
      else if (route.needsValue && disp !== "ignore") inner.push(ret(ranValue()));
      else if (disp === "ignore") inner.push(expr(num(1)));
      else inner.push(expr(ranValue()));
      const e = prim(host, fn(inner), extra);
      if (disp === "return" || disp === "store") {
        const api = name(ctx, "u");
        out.body.push(decl(api, e));
        handlerFor(escape(cur, ref(api)), null);
      } else if (route.needsValue && disp !== "ignore") {
        const api = name(ctx, "u");
        out.body.push(decl(api, e));
        const id = viewId();
        out.view.push(child(id, ref(api)));
        withWitness(id);
      } else {
        out.body.push(expr(e));
        out.wrappable = true;
      }
      return out;
    }
    case "body": {
      out.body.push(...pre);
      if (disp === "handler") {
        // The closure is declared in the component body and called later from a click.
        let access = cur;
        if (cur.e.k !== "ref") { const id = name(ctx, "f"); out.body.push(decl(id, cur.e)); access = { ...cur, e: ref(id) }; }
        handlerFor(access, null);
      } else if (disp === "ignore") { /* declared and left alone */ } else if (route.needsValue) {
        const id = name(ctx, "n");
        out.body.push(decl(id, ranValue()));
        const vid = viewId();
        out.view.push(child(vid, ref(id)));
        withWitness(vid);
      } else out.body.push(expr(ranValue()));
      return out;
    }
    case "jsx": {
      const id = viewId();
      const e = disp === "ignore" ? num(0) : ranValue();
      const full = pre.length ? iife(fn([...pre, ret(e)])) : e;
      out.view.push(!route.needsValue && ctx.rng.chance(0.25) ? attr(id, full) : child(id, full));
      if (route.needsValue) setReadWitness(cur, pre, viewWitness(id, route.init));
      return out;
    }
    case "apply": case "settled": case "cleanup": case "timer": case "then": case "handler": case "async": {
      const stmts = [...pre];
      if (disp === "return") stmts.push(ret(cur.e));
      else if (disp === "ignore") { /* built, not used */ } else stmts.push(sinkStmt(route, ranValue()));
      if (host === "handler") {
        out.view.push(button(name(ctx, "b"), stmts));
      } else if (host === "async") {
        const f = name(ctx, "f");
        out.body.push(decl(f, fn([awaitStmt(), ...stmts], { async: true })));
        if (ctx.rng.chance(0.6)) out.body.push(expr(call(ref(f))));
        else out.view.push(button(name(ctx, "b"), [expr(call(ref(f)))]));
        out.wrappable = false;
      } else {
        const wrapped = host === "apply" ? prim("effect", fn([ret(num(1))]), { apply: fn(stmts) })
          : prim(host === "settled" ? "settled" : host, fn(stmts));
        out.body.push(expr(wrapped));
        out.wrappable = true;
      }
      return out;
    }
    default: throw new Error(`unknown host ${host}`);
  }
}

/** The witness of a read is the page text shown once its value reaches the sink, so a
 * route through a view child replaces the sink probe's witness on the payload node. */
function setReadWitness(cur, pre, witness) {
  const target = findPayload([cur.e, ...pre]);
  if (target) target.witness = witness;
}
function findPayload(nodes) {
  const stack = [...nodes];
  while (stack.length) {
    const node = stack.pop();
    if (!node || typeof node !== "object") continue;
    if (isSite(node)) return node;
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) stack.push(...value);
      else if (value && typeof value === "object") stack.push(value);
    }
  }
  return null;
}

/** One more scope around a leaf subject: the component body becomes the body of an outer host. */
function wrapOuter(ctx, body, view) {
  const { rng } = ctx;
  const outer = rng.pick(["untrack", "root", "owner", "memo", "settled", "timer", "then", "apply", "effect"]);
  // The subject must reach no body-level name from the view: only probes, which read signals.
  if (view.some((item) => item.k !== "probe")) return body;
  switch (outer) {
    case "memo": {
      const api = name(ctx, "m");
      view.push(child(name(ctx, "v"), call(ref(api))));
      return [decl(api, prim("memo", fn([...body, ret(num(1))])))];
    }
    case "effect": return [expr(prim("effect", fn([...body, ret(num(1))]), { apply: fn([]) }))];
    case "apply": return [expr(prim("effect", fn([ret(num(1))]), { apply: fn(body) }))];
    case "owner": return [expr(prim("owner", fn(body), { ownerArg: "null" }))];
    default: return [expr(prim(outer, fn(body)))];
  }
}

/** A legal construct with its own site: a tracked read in a memo, a write in an apply, or a write in a handler. */
function addDecoy(ctx, sigs, body, view) {
  const { rng } = ctx;
  const kind = rng.pick(["memo-read", "apply-write", "handler-write"]);
  if (kind === "memo-read") {
    const init = 3 + counter(ctx, "decoy");
    const src = signal(name(ctx, "d"), init, "src");
    sigs.push(src);
    const api = name(ctx, "m");
    const id = name(ctx, "v");
    body.push(decl(api, prim("memo", fn([ret(read(src.name, { site: true, witness: viewWitness(id, init) }))]))));
    view.push(child(id, call(ref(api))));
    return;
  }
  const value = 40 + 3 * counter(ctx, "decoy");
  const target = signal(name(ctx, "w"), 0, "probe");
  sigs.push(target);
  const legal = write(target.name, value, { site: true, witness: probeWitness(target.name, value) });
  view.push(probe(target.name));
  if (kind === "apply-write") {
    const driver = signal(name(ctx, "d"), 1, "src");
    sigs.push(driver);
    body.push(expr(prim("effect", fn([ret(read(driver.name))]), { apply: fn([expr(legal)]) })));
  } else view.push(button(name(ctx, "b"), [expr(legal)]));
}
