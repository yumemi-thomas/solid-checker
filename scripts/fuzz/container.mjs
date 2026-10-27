// The containers dimension: places where code is written but runs elsewhere, later, or not at all.
//
// A generated program is a tree of statements. This pass wraps expression statements of a finished component in a *container*
// (model.mjs `contain`) and rewrites some of what is left: it is where the checker's downgrade and safety logic lives (a class
// field initializer, a component nobody renders, dead code, a callback passed by name), and where a reviewer finds the false
// positives a flat program cannot show. The pass changes nothing the generator decided: it only moves statements into a scope.
//
//   function components   an uppercase inner function: never rendered, rendered as <Child />, passed to a slot (a local object's
//                         method that calls it, or an optional `props.slot?.(Child)` the harness never supplies), or returned
//   class bodies          an instance field initializer, a static field, a static block, a method, a constructor; the class
//                         constructed (and the method called) or never constructed
//   dead code             `if (false)`, after `return`, a function never called, `false && f()` (and `true && f()` as the live twin)
//   callbacks by name     the callback a primitive is handed is declared before it (const, let or a function declaration) and
//                         passed by name: onCleanup, createMemo, createEffect, createRoot, runWithOwner, untrack, onSettled,
//                         queueMicrotask, setTimeout, .then, [1].forEach/map and the arguments of a same-file helper
//   call forms            `f?.()`, `(f as typeof f)()`, `(0, f)()`, `(true && f)()`, `(f)()`
//   async gaps            an `await` before or after the payload inside a function-like container
//
// Every container is valid TypeScript (the type check is the authority) and the hit ledger still tells ran from not-ran: a
// function-like container has an entry counter, a direct form (`f = h();`, `false && h();`) has the glue counter of its call.
import { contain, awaitStmt, ret, nul, jsx, decl, ref, walk, slotsOf, ASYNC_KINDS, USES } from "./model.mjs";
import { createRng } from "./rng.mjs";

const STATEMENTS = new Set(["expr", "decl", "return", "await", "try", "contain"]);

/** Container kinds and their weights; `weights` overrides per kind. */
export const KIND_WEIGHTS = [
  [3, "comp-never"], [3, "comp-rendered"], [2, "comp-slot"], [2, "comp-prop"], [2, "comp-returned"],
  [3, "class-field"], [1, "class-static"], [1, "class-sblock"], [2, "class-method"], [2, "class-ctor"],
  [1, "dead-if"], [1, "dead-after-return"], [1, "dead-fn"], [1, "and-true"], [1, "and-false"],
];
const BINDINGS = [[5, "const"], [2, "let"], [3, "function"]];
const VIAS = [[3, "optional"], [2, "as"], [2, "comma"], [1, "and"], [1, "paren"]];

const statementLists = (root) => {
  const lists = [];
  for (const { node } of walk(root)) {
    for (const [key, isList] of slotsOf(node)) if (isList && node[key].every((item) => STATEMENTS.has(item.k))) lists.push(node[key]);
  }
  return lists;
};

const plainCallee = (callee) => callee.k === "ref" || (callee.k === "member" && plainCallee(callee.obj));

/** How interesting a statement is to wrap: a call of a name (a cleanup handle) and the scopes it opens first. */
function interest(stmt) {
  const e = stmt.e;
  if (e.k === "call" && e.callee.k === "ref") return 4;
  if (e.k === "prim" || e.k === "own" || e.k === "iife" || e.k === "iter") return 2;
  return 1;
}

/**
 * Wraps, rewrites and names. Mutates `component`. `ctx` has `counters` (the generator's name counters); `rng` is a stream of its own, so
 * a program without containers is the program it always was.
 * @param {{ counters: object }} ctx
 * @param {object} component
 * @param {{ rng: object, kinds?: [number, string][], count?: [number, number][] }} options
 */
export function applyContainers(ctx, component, { rng, kinds = KIND_WEIGHTS, count = [[2, 0], [5, 1], [3, 2]] } = {}) {
  const next = (key) => { ctx.counters[key] = (ctx.counters[key] ?? 0) + 1; return ctx.counters[key]; };
  const wanted = rng.weighted(count);
  for (let made = 0; made < wanted; made += 1) {
    const candidates = [];
    for (const list of statementLists(component)) list.forEach((stmt, index) => { if (stmt.k === "expr") candidates.push({ list, stmt, index, weight: interest(stmt) }); });
    if (!candidates.length) break;
    const chosen = rng.weighted(candidates.map((entry) => [entry.weight, entry]));
    const kind = rng.weighted(kinds);
    wrapAt(chosen.list, chosen.index, kind, next("container"), {
      take: list2(chosen.list, chosen.index) && rng.chance(0.2) ? 2 : 1,
      async: ASYNC_KINDS.has(kind) && rng.chance(0.35), gap: rng.chance(0.5) ? "before" : "after", returns: rng.chance(0.25) ? "null" : "jsx", use: USES[kind] ? rng.pick(USES[kind]) : null,
    });
  }
  // Rewrites run on the lists as they stand now (a container's body is a list too).
  for (const list of statementLists(component)) {
    for (const stmt of list) if (stmt.k === "expr" && stmt.e.k === "call" && plainCallee(stmt.e.callee) && rng.chance(0.3)) stmt.e.via = rng.weighted(VIAS.map(([w, v]) => [w, v]));
  }
  for (const list of statementLists(component)) hoistList(list, rng, () => `cb${next("cb")}`);
  return component;
}

const list2 = (list, index) => list[index + 1]?.k === "expr";

/**
 * Replaces `take` statements of `list` from `index` by one container of `kind`. A function-like container may be async, with its `await`
 * `before` or `after` the statements; a component returns `<span />` (or `null`, `returns: "null"`: the checker reads a function that returns JSX as a component). Exported for the pinned shapes of the tests.
 */
export function wrapAt(list, index, kind, id, { take = 1, async: isAsync = false, gap = "before", use = USES[kind]?.[0] ?? null, returns = "jsx" } = {}) {
  const body = list.slice(index, index + take);
  const inner = isAsync ? (gap === "before" ? [awaitStmt(), ...body] : [...body, awaitStmt()]) : body;
  const full = kind.startsWith("comp") ? [...inner, ret(returns === "null" ? nul() : jsx())] : inner;
  const made = contain(kind, id, full, { async: isAsync, use });
  list.splice(index, take, made);
  return made;
}

/** The callback slots of a statement's expression: [holder, key] pairs whose value is a function literal. */
function callbackSlots(top) {
  const slots = [];
  if (top.k === "prim" && top.name !== "rootd") { slots.push([top, "fn"]); if (top.apply) slots.push([top, "apply"]); }
  else if (top.k === "own" && top.op !== "context") { slots.push([top, "fn"]); if (top.apply) slots.push([top, "apply"]); }
  else if (top.k === "iter") slots.push([top, "fn"]);
  else if (top.k === "call" && top.callee.k === "ref") top.args.forEach((arg, index) => { if (arg.k === "fn") slots.push([top.args, index]); });
  return slots;
}

const mentions = (root, name) => [...walk(root)].some(({ node }) => node.k === "ref" && node.name === name);

function hoistList(list, rng, nameFor) {
  for (let index = 0; index < list.length; index += 1) {
    const stmt = list[index];
    if ((stmt.k !== "expr" && stmt.k !== "decl") || !stmt.e) continue;
    const before = [];
    for (const [holder, key] of callbackSlots(stmt.e)) {
      const f = holder[key];
      if (f?.k !== "fn" || f.params.length || !rng.chance(0.4)) continue;
      // An effect's apply takes the compute's value as `v`: a declared function has no such parameter.
      if (key === "apply" && mentions(f, "v")) continue;
      const binding = rng.weighted(BINDINGS);
      const name = nameFor();
      holder[key] = ref(name);
      before.push(decl(name, { ...f, form: binding === "function" ? "function" : "arrow" }, binding === "let" ? { kw: "let" } : {}));
    }
    if (before.length) { list.splice(index, 0, ...before); index += before.length; }
  }
}

/**
 * Per container: its kind, how it is used, and how often its function was entered (null for the direct forms and when the run was
 * not instrumented). `entered: 0` is a container that never ran.
 */
export function containerEvidence(program, ledger, hits) {
  const rows = [];
  for (const { node, path } of walk(program)) {
    if (node.k !== "contain") continue;
    const entry = (ledger ?? []).find((row) => row.kind === "entry" && row.node === "contain" && row.path.length === path.length && row.path.every((part, at) => String(part) === String(path[at])));
    rows.push({ kind: node.kind, use: node.use ?? undefined, async: node.async || undefined, entered: entry && hits ? Number(hits[entry.hit] ?? 0) : null });
  }
  return rows;
}

/** The stream of a seed's containers: independent of the generator's own, so opting in leaves every other draw alone. */
export const containerRng = (seed) => createRng(((Number(seed) >>> 0) ^ 0x636f6e74) >>> 0);
