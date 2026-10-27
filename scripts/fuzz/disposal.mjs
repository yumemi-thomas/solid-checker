// The disposal profile: a write that runs while an owner is being disposed.
//
// rc.17's owned-scope write guards (setSignal, markRefresh, the store setter) throw REACTIVE_WRITE_IN_OWNED_SCOPE
// unless `disposalDepth > 0`, and `runDisposal` raises that depth around the callbacks an owner registered with
// `onCleanup`. An action call has no such exemption (ACTION_CALLED_IN_OWNED_SCOPE). An effect's returned cleanup runs
// before a rerun outside disposal, and at final teardown inside it. An `await` ends the disposal, and the handle
// `onCleanup` returns runs the callback later, outside any disposal. This profile builds one component with:
//
//   a host owner the driver can dispose   a Show toggled by a button, a memo or effect a button reruns, a createRoot
//                                         whose disposer a button calls, or an effect in the component body a button reruns
//   a cleanup in that host                `onCleanup(cb)` with its handle discarded, kept, called at once or called later
//                                         from a button, or the cleanup an effect's apply returns
//   a chain around the write              runWithOwner(captured | createOwner() | null), createRoot, untrack, an IIFE,
//                                         forEach, a named or stored function, a helper, a timer, a microtask, a promise
//                                         callback, an async function (await before or after), try/finally, flush()
//   the write                             a signal setter, a store setter, `refresh(memo)`, or an action call
//
// Controls put the same chain in a component body (the write throws there) or an event handler (legal).
// The write is the only site of interest; the cleanup callback and the host are marked (`role`) so the report can
// say whether the driver really disposed the host: a site that never ran proves nothing.
import {
  signal, storeSignal, memoSignal, actionSignal, num, read, write, ref, member, obj, call, iife, iter, fn, prim, expr, decl, ret,
  awaitStmt, tryStmt, flush, probe, button, child, show, getOwner, own,
} from "./model.mjs";

const probeWitness = (sig, value) => ({ selector: `[data-probe="${sig}"]`, text: String(value) });

/** The names of the chain wrappers, weighted. */
const WRAPPERS = [
  [3, "owner-captured"], [2, "owner-create"], [2, "owner-null"], [2, "root"], [1, "root-named"], [2, "untrack"], [2, "iife"], [1, "forEach"],
  [2, "named-call"], [1, "stored"], [1, "method"], [1, "helper"], [2, "timer"], [1, "micro"], [1, "then"], [2, "async-before"], [1, "async-after"],
  [1, "try-body"], [1, "try-finally"], [1, "flush-before"], [1, "flush-after"],
];

/**
 * Builds the disposal component's parts.
 * @param {{ rng: object, name: (prefix: string) => string, counter: (key: string) => number, ensureHelper: (form: string) => string }} kit
 * @returns {{ sigs: object[], body: object[], view: object[] }}
 */
export function buildDisposal(kit, plan = {}) {
  const { rng, name, counter } = kit;
  const sigs = [];
  const view = [];
  const body = [];
  const value = 10 + 7 * counter("value");

  // 1. The write and its witness.
  const via = plan.via ?? rng.weighted([[5, "signal"], [2, "store"], [2, "refresh"], [2, "action"]]);
  let theWrite;
  if (via === "refresh") {
    const target = memoSignal(name("m"));
    sigs.push(target);
    theWrite = write(target.name, 0, { site: true, witness: null, via });
  } else {
    const target = via === "store" ? storeSignal(name("w"), 0) : signal(name("w"), 0, "probe");
    sigs.push(target);
    view.push(probe(target.name));
    if (via === "action") {
      const act = actionSignal(name("a"), target.name, value);
      sigs.push(act);
      theWrite = write(act.name, 0, { site: true, witness: probeWitness(target.name, value), via });
    } else theWrite = write(target.name, value, { site: true, witness: probeWitness(target.name, value), via });
  }

  // 2. Where it sits and how the driver disposes the host.
  const control = "control" in plan ? plan.control : rng.chance(0.12) ? rng.pick(["control-body", "control-handler"]) : null;
  const where = control ? null : plan.where ?? rng.weighted([[4, "show"], [3, "memo"], [2, "effect"], [3, "rootd"], [2, "body"]]);
  const cleanupVia = where === "effect" ? "onCleanup" : where === "body" ? "returned" : plan.cleanupVia ?? rng.weighted([[3, "onCleanup"], [2, "returned"]]);
  const handleMode = cleanupVia === "returned" ? "none"
    : plan.handleMode ?? rng.weighted([[5, "discard"], [1, "keep"], [2, "call-now"], [where === "memo" || where === "rootd" ? 2 : 0, "call-later"]]);
  const trigger = signal(name("t"), 0, "src");
  const flag = signal(name("o"), 1, "src");
  const usesTrigger = "usesTrigger" in plan ? plan.usesTrigger : where === "memo" || where === "effect" || where === "body" || (cleanupVia === "returned" && rng.chance(0.5));

  // 3. The chain around the write, innermost first.
  const wrappers = [];
  if (plan.wrappers) wrappers.push(...plan.wrappers);
  else {
    const depth = rng.weighted([[2, 0], [4, 1], [3, 2], [1, 3]]);
    for (let i = 0; i < depth; i += 1) wrappers.push(rng.weighted(WRAPPERS));
  }
  const needsOwner = wrappers.includes("owner-captured");
  let chain = [expr(theWrite)];
  for (const wrapper of wrappers) chain = wrap(kit, wrapper, chain);

  const cleanupFn = fn(chain);
  cleanupFn.role = "disposal-cleanup";
  const capture = needsOwner ? [decl("owner", getOwner())] : [];

  if (control) {
    if (control === "control-body") body.push(...capture, ...chain);
    else {
      view.push(button(name("b"), [...capture, ...chain]));
    }
    sigs.push(trigger);
    return { sigs, body, view, shape: { via, control, wrappers, handleMode: "none", cleanupVia: "none" } };
  }

  // 4. The cleanup in the host.
  let statements;
  let handleName = null;
  if (cleanupVia === "returned") {
    const compute = usesTrigger ? [expr(read(trigger.name)), ret(num(1))] : [ret(num(1))];
    statements = [...capture, expr(prim("effect", fn(compute), { apply: fn([ret(cleanupFn)]) }))];
  } else if (handleMode === "discard") statements = [...capture, expr(own("cleanup", { fn: cleanupFn }))];
  else {
    handleName = name("h");
    statements = [...capture, decl(handleName, own("cleanup", { fn: cleanupFn, handle: true }))];
    if (handleMode === "call-now") statements.push(expr(call(ref(handleName))));
  }

  // 5. The host and its driver.
  const bump = () => button(name("b"), [expr(write(trigger.name, 1))]);
  sigs.push(trigger);
  switch (where) {
    case "show": {
      sigs.push(flag);
      view.push(show(read(flag.name), [child(name("v"), iife(fn([...statements, ret(num(1))])))]));
      view.push(button(name("b"), [expr(write(flag.name, 0))]));
      if (usesTrigger) view.push(bump());
      break;
    }
    case "memo": {
      const api = name("m");
      const later = handleMode === "call-later";
      body.push(decl(api, prim("memo", fn([expr(read(trigger.name)), ...statements, ret(later ? ref(handleName) : num(1))]))));
      if (later) view.push(button(name("b"), [expr(call(call(ref(api))))]));
      else view.push(child(name("v"), call(ref(api))));
      view.push(bump());
      break;
    }
    case "effect": {
      body.push(expr(prim("effect", fn([expr(read(trigger.name)), ...statements, ret(num(1))]), { apply: fn([]) })));
      view.push(bump());
      break;
    }
    case "rootd": {
      const api = name("r");
      const later = handleMode === "call-later";
      body.push(decl(api, prim("rootd", fn([...statements, ret(later ? ref(handleName) : ref("dispose"))]))));
      view.push(button(name("b"), [expr(call(ref(api)))]));
      if (usesTrigger) view.push(bump());
      break;
    }
    default: {
      body.push(...statements);
      view.push(bump());
    }
  }
  return { sigs, body, view, shape: { via, where, cleanupVia, handleMode, wrappers, usesTrigger } };
}

/** One chain wrapper: the statements that run `inner` the wrapper's way. */
function wrap(kit, wrapper, inner) {
  const { name, ensureHelper } = kit;
  const f = () => name("f");
  switch (wrapper) {
    case "owner-captured": return [expr(prim("owner", fn(inner), { ownerArg: "owner" }))];
    case "owner-create": return [expr(prim("owner", fn(inner), { ownerArg: "create" }))];
    case "owner-null": return [expr(prim("owner", fn(inner), { ownerArg: "null" }))];
    case "root": return [expr(prim("root", fn(inner)))];
    case "root-named": { const id = f(); return [decl(id, fn(inner)), expr(prim("root", ref(id)))]; }
    case "untrack": return [expr(prim("untrack", fn(inner)))];
    case "iife": return [expr(iife(fn(inner)))];
    case "forEach": return [expr(iter("forEach", fn(inner)))];
    case "named-call": { const id = f(); return [decl(id, fn(inner)), expr(call(ref(id)))]; }
    case "stored": { const id = name("s"); return [decl(id, obj([["f", fn(inner)]])), expr(call(member(ref(id), "f")))]; }
    case "method": { const id = name("s"); return [decl(id, obj([["f", fn(inner, { form: "method" })]])), expr(call(member(ref(id), "f")))]; }
    case "helper": return [expr(call(ref(ensureHelper("call")), [fn(inner)]))];
    case "timer": return [expr(prim("timer", fn(inner)))];
    case "micro": return [expr(prim("micro", fn(inner)))];
    case "then": return [expr(prim("then", fn(inner)))];
    case "async-before": { const id = f(); return [decl(id, fn([awaitStmt(), ...inner], { async: true })), expr(call(ref(id)))]; }
    case "async-after": { const id = f(); return [decl(id, fn([...inner, awaitStmt()], { async: true })), expr(call(ref(id)))]; }
    case "try-body": return [tryStmt(inner, [])];
    case "try-finally": return [tryStmt([], inner)];
    case "flush-before": return [expr(flush()), ...inner];
    case "flush-after": return [...inner, expr(flush())];
    default: throw new Error(`unknown wrapper ${wrapper}`);
  }
}

/**
 * Whether the driver really ran the cleanup: the hit count of the entry counter of the function marked
 * `role: "disposal-cleanup"` (null when the program has none or the run was not instrumented).
 * The write's own hit count says the write ran; this says the callback that holds it was entered at all.
 */
export function cleanupEvidence(program, ledger, hits) {
  let marked = null;
  const stack = [{ node: program, path: [] }];
  while (stack.length && !marked) {
    const { node, path } = stack.pop();
    if (node && typeof node === "object") {
      if (node.k === "fn" && node.role === "disposal-cleanup") { marked = path; break; }
      for (const [key, value] of Object.entries(node)) if (value && typeof value === "object") stack.push({ node: value, path: [...path, key] });
    }
  }
  if (!marked || !hits) return null;
  const entry = (ledger ?? []).find((row) => row.kind === "entry" && row.path.length === marked.length && row.path.every((part, index) => String(part) === String(marked[index])));
  return entry ? { hit: entry.hit, line: entry.line, entered: Number(hits[entry.hit] ?? 0) } : null;
}
