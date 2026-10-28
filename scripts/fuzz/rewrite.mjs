// Rewrites that do not change what a program does at run time (equivalence mode, equivalence.mjs).
//
// Metamorphic testing: the checker's verdict at a site must not depend on how the same behaviour is written. Each rewrite below
// turns a program tree into another tree that does exactly what the first does under rc.17, and keeps every site of interest
// (the printer's `s1..sN`) as the same node, tagged `eqSite` with its original id: the site map is the tag. A rewrite never moves
// code into or out of an owner, a tracking scope, an effect, `untrack`, a component body or an async boundary, and never changes
// when anything runs, so none depends on timing, owner context or tracking. Every rewrite states its soundness argument next to it;
// if Chrome ever shows a rewrite changing behaviour, the rewrite is wrong (a fuzzer bug) and is removed.
//
// A rewrite is `{ kind, sound, candidates(program, fresh) }`; `candidates` lists the places it can apply to in a (cloned) tree, each
// `{ describe, apply() }`, where `apply` returns true when it changed the tree. The statement-level rewrites are decorations the
// printer renders (`stmt.wrap`, `stmt.extract`, `call.via`, `read/write.eqVia`; print.mjs): the tree keeps its shape, so paths and sites hold.
import { clone, isSite, walk } from "./model.mjs";

/** The tag a site node carries to its original id. */
export const SITE_TAG = "eqSite";

/** A copy of `program` with every site node tagged by the id the printer gives it (`s<n>`, in walk order). */
export function tagSites(program) {
  const copy = clone(program);
  let index = 0;
  for (const { node } of walk(copy)) if (isSite(node)) { index += 1; node[SITE_TAG] = `s${index}`; }
  return copy;
}

/** Statement lists of a tree: [list, parent node]. */
function* statementLists(program) {
  for (const { node } of walk(program)) {
    switch (node.k) {
      case "component": case "fn": case "contain": yield [node.body, node]; break;
      case "try": yield [node.body, node]; yield [node.fin, node]; break;
      case "button": yield [node.handler, node]; break;
      default: break;
    }
  }
}

/** A container that prints its single expression statement bare (`f = expr;`, `true && expr;`): the statement's own text is not printed. */
const printsBare = (parent, list) => parent.k === "contain" && ["class-field", "class-static", "and-true", "and-false"].includes(parent.kind) && list.length === 1 && list[0].k === "expr";

/** Expression kinds a statement may hold for a wrapper: calls the printer writes as one expression statement. */
const WRAPPABLE = new Set(["call", "iife", "iter", "prim", "read", "write", "own", "set", "getOwner", "use"]);
const wrappable = (stmt) => stmt.k === "expr" && WRAPPABLE.has(stmt.e.k) && !stmt.wrap && !stmt.extract;
/** A `return` can sit in a block, an `if (true)` or a `try`/`finally {}` (the function still returns the same value); it cannot be called as a function. */
const wrappableControl = (stmt) => wrappable(stmt) || (stmt.k === "return" && !stmt.wrap && !stmt.extract);

/** Every `ref` name in a subtree. */
const refsIn = (node) => new Set([...walk(node)].filter(({ node: n }) => n.k === "ref").map(({ node: n }) => n.name));

/** Candidates that wrap an eligible expression statement; `wrapper(name)` is the printer's wrapper record. */
function wrapCandidates(program, fresh, kind, wrapper, { control = false } = {}) {
  const out = [];
  for (const [list, parent] of statementLists(program)) {
    if (printsBare(parent, list)) continue;
    for (const stmt of list) {
      if (!(control ? wrappableControl(stmt) : wrappable(stmt))) continue;
      const index = out.length;
      out.push({ describe: `${kind} around ${stmt.k} ${stmt.e.k}`, apply: () => { if (stmt.wrap) return false; stmt.wrap = [wrapper(fresh(), index)]; return true; } });
    }
  }
  return out;
}

const SIGNAL_FREE = new Set(["owner", "dispose", "v"]);
/** The generated function and value names a rename may change: declared by `decl` or a same-file helper (never the reserved `owner`, a package export, a parameter). */
function renameTargets(program) {
  const names = new Set();
  for (const { node } of walk(program)) {
    if (node.k === "decl") names.add(node.name);
    if (node.k === "helper" && !node.pkg) names.add(node.name);
  }
  for (const name of SIGNAL_FREE) names.delete(name);
  // A `ref` can also name a signal's accessor or setter; a name shared with one is left alone.
  for (const { node } of walk(program)) if (node.k === "signal") { names.delete(node.name); names.delete(node.setter); }
  return names;
}

const VIAS = ["optional", "paren", "comma", "as", "and"];
/** A call form chosen by position, so a rewrite is a function of the program and its choices only. */
const pickVia = (index) => VIAS[index % VIAS.length];

/**
 * The rewrites. `sound` is the argument that the rewritten program does the same thing.
 */
export const REWRITES = {
  rename: {
    sound: "A local name (a `decl`, a same-file helper) is only a binding: every reference is renamed with it, nothing is renamed to a name already in use (`v_` + the old name), and `owner` (printed as a literal by `runWithOwner`) and parameters are left alone. Alpha-equivalence.",
    candidates(program) {
      const names = renameTargets(program);
      const taken = new Set([...walk(program)].flatMap(({ node }) => (node.k === "decl" || node.k === "ref" || node.k === "helper" ? [node.name] : [])));
      const out = [];
      for (const name of [...names].sort()) {
        const next = `v_${name}`;
        if (taken.has(next)) continue;
        out.push({ describe: `rename ${name}`, apply: () => {
          for (const { node } of walk(program)) if ((node.k === "decl" || node.k === "ref" || node.k === "helper") && node.name === name && !node.pkg) node.name = next;
          return true;
        } });
      }
      return out;
    },
  },

  extract: {
    sound: "`e;` and `const t = e;` evaluate `e` once, at the same point, in the same scope; `t` is a fresh name nothing reads. For a `return e;`, `const t = e; return t;` returns the same value. Only a statement whose expression is a call (no function literal whose contextual type could change) is rewritten.",
    candidates(program, fresh) {
      const out = [];
      for (const [list, parent] of statementLists(program)) {
        if (printsBare(parent, list)) continue;
        for (const stmt of list) {
          if (!(stmt.k === "expr" || stmt.k === "return") || stmt.extract || stmt.wrap || !WRAPPABLE.has(stmt.e.k)) continue;
          out.push({ describe: `extract ${stmt.k} ${stmt.e.k}`, apply: () => { if (stmt.extract) return false; stmt.extract = fresh(); return true; } });
        }
      }
      return out;
    },
  },

  block: {
    sound: "`{ s }` is `s` in a block scope; an expression statement declares nothing, so no binding changes visibility, and control flow is untouched.",
    candidates: (program, fresh) => wrapCandidates(program, fresh, "block", () => ({ w: "block" }), { control: true }),
  },

  "if-true": {
    sound: "`if (true) { s }` always runs `s`, once, at the same point. The condition is a literal with no effects, so no guard is real; this asks the checker to see through one.",
    candidates: (program, fresh) => wrapCandidates(program, fresh, "if-true", () => ({ w: "if-true" }), { control: true }),
  },

  "try-finally": {
    sound: "`try { s } finally {}` runs `s` and then nothing; a throw from `s` propagates unchanged (the empty `finally` neither catches nor swallows), and a `return` inside still returns its value (the empty `finally` cannot override it).",
    candidates: (program, fresh) => wrapCandidates(program, fresh, "try-finally", () => ({ w: "try-finally" }), { control: true }),
  },

  iife: {
    sound: "`(() => { s })();` calls a synchronous arrow with no parameters at once, exactly once, at the statement's place. A plain call changes neither the owner nor the tracking listener (both are set only by Solid primitives, not by calling a function), `this` and `arguments` are not used, and `s` holds no `return`, `await` or declaration the wrapper would capture.",
    candidates: (program, fresh) => wrapCandidates(program, fresh, "iife", () => ({ w: "iife" })),
  },

  outline: {
    sound: "`const h = () => { s }; h();` (or `function h() { s } h();`) declares a helper and calls it at once, exactly once, in the statement's place: the same argument as an immediate call. The declaration is directly before its only call, so hoisting does not matter.",
    // The declaration form alternates by position: an arrow in a const, then a function declaration.
    candidates: (program, fresh) => wrapCandidates(program, fresh, "outline", (name, index) => ({ w: index % 2 === 0 ? "outline-arrow" : "outline-function", name })),
  },

  inline: {
    sound: "A statement `(() => { s1; s2 })();` whose arrow is synchronous with no parameters and whose body is only expression and declaration statements runs `s1; s2` once, in order, at that place; splicing them into the enclosing list does the same. Declared names are unique in the program, so none can capture or be captured.",
    candidates(program) {
      const out = [];
      // Only the outermost of nested IIFEs: splicing the outer moves the inner statement to a list the inner candidate no longer holds.
      const inlined = new Set();
      for (const [list, parent] of statementLists(program)) {
        if (printsBare(parent, list) || parent.k === "contain" || inlined.has(parent)) continue;
        for (const stmt of list) {
          const f = stmt.k === "expr" && !stmt.wrap && !stmt.extract && stmt.e.k === "iife" ? stmt.e.fn : null;
          if (!f || f.k !== "fn" || f.async || f.params.length || f.form !== "arrow") continue;
          inlined.add(f);
          if (!f.body.every((inner) => (inner.k === "expr" || (inner.k === "decl" && !(inner.e.k === "fn" && inner.e.form === "function"))))) continue;
          out.push({ describe: "inline iife", apply: () => {
            const at = list.indexOf(stmt);
            if (at < 0) return false;
            list.splice(at, 1, ...f.body);
            return true;
          } });
        }
      }
      return out;
    },
  },

  callform: {
    sound: "`f()`, `f?.()`, `(f)()`, `(void 0, f)()`, `(f as typeof f)()` and `(true && f)()` call the same function with the same arguments and `this` (undefined for a plain name). `f` is a declared function, never nullish, so `?.` always calls. Only a plain name is rewritten, never a member (the comma and `&&` forms would drop the receiver).",
    candidates(program) {
      const names = renameTargets(program);
      const out = [];
      let index = 0;
      for (const { node } of walk(program)) {
        if (node.k !== "call" || node.via || node.callee.k !== "ref" || !names.has(node.callee.name)) continue;
        const via = pickVia(index += 1);
        out.push({ describe: `call ${node.callee.name} ${via}`, apply: () => { if (node.via) return false; node.via = via; return true; } });
      }
      return out;
    },
  },

  "accessor-form": {
    sound: "The same call forms as `callform`, on a signal accessor or setter (`count()`, `setCount(1)`), a store setter, `refresh(m)` or an action: the callee is an identifier bound to a function, so the call, its arguments and its `this` are unchanged.",
    candidates(program) {
      const out = [];
      let index = 0;
      for (const { node } of walk(program)) {
        if ((node.k !== "read" && node.k !== "write") || node.eqVia) continue;
        const via = pickVia(index += 1);
        out.push({ describe: `${node.k} ${node.sig} ${via}`, apply: () => { if (node.eqVia) return false; node.eqVia = via; return true; } });
      }
      return out;
    },
  },

  declform: {
    sound: "`const f = () => {...}` and `function f() {...}` bind the same closure; the generated bodies use neither `this` nor `arguments`. Arrow to function adds hoisting (every use stays valid). Function to arrow is done only when every reference to the name comes textually after the declaration, so no use can reach the temporal dead zone.",
    candidates(program) {
      const order = new Map();
      let at = 0;
      for (const { node } of walk(program)) order.set(node, at += 1);
      const refsByName = new Map();
      for (const { node } of walk(program)) if (node.k === "ref") refsByName.set(node.name, [...(refsByName.get(node.name) ?? []), node]);
      const out = [];
      for (const [list] of statementLists(program)) {
        for (const stmt of list) {
          if (stmt.k !== "decl" || stmt.kw || stmt.e.k !== "fn" || !["arrow", "function"].includes(stmt.e.form) || !/^[a-z]/.test(stmt.name)) continue;
          const toFunction = stmt.e.form === "arrow";
          const laterOnly = (refsByName.get(stmt.name) ?? []).every((use) => order.get(use) > order.get(stmt));
          if (!toFunction && !laterOnly) continue;
          out.push({ describe: `${stmt.name} ${toFunction ? "arrow to function" : "function to arrow"}`, apply: () => { stmt.e.form = toFunction ? "function" : "arrow"; return true; } });
        }
      }
      return out;
    },
  },

  reorder: {
    sound: "Creating a closure has no effect and reads nothing at creation, so a `const f = () => ...` can swap places with the statement next to it when that statement does not mention `f` and (if it declares a name) `f`'s body does not mention that name: no read moves across a write, and no use can precede its declaration. A `return` is never swapped.",
    candidates(program) {
      const out = [];
      for (const [list, parent] of statementLists(program)) {
        if (parent.k === "contain") continue;
        for (let at = 0; at + 1 < list.length; at += 1) {
          const [a, x] = [list[at], list[at + 1]];
          if (a.k !== "decl" || a.kw || a.e.k !== "fn" || x.k === "return") continue;
          out.push({ describe: `swap ${a.name} with the next statement`, apply: () => {
            const i = list.indexOf(a);
            if (i < 0 || list[i + 1] !== x) return false;
            if (refsIn(x).has(a.name)) return false;
            if (x.k === "decl" && refsIn(a).has(x.name)) return false;
            list[i] = x; list[i + 1] = a;
            return true;
          } });
        }
      }
      return out;
    },
  },

  "dead-decl": {
    sound: "A fresh declaration nothing reads (a number, an arrow never called, a function declaration never called) has no effect when its initializer is a literal or a function expression, and no other statement mentions its name.",
    candidates(program, fresh) {
      const out = [];
      let index = 0;
      for (const [list, parent] of statementLists(program)) {
        if (parent.k === "contain" || parent.k === "try" || parent.k === "button") continue;
        const limit = list.at(-1)?.k === "return" ? list.length - 1 : list.length;
        const form = (index += 1) % 3;
        out.push({ describe: `dead declaration ${["number", "arrow", "function"][form]}`, apply: () => {
          const name = fresh();
          const at = Math.min(limit, Math.floor(list.length / 2));
          const e = form === 0 ? { k: "num", n: 7 } : { k: "fn", form: form === 1 ? "arrow" : "function", async: false, params: [], body: [] };
          list.splice(at, 0, { k: "decl", name, e });
          return true;
        } });
      }
      return out;
    },
  },
};

export const REWRITE_KINDS = Object.keys(REWRITES);

/** The kinds that apply to a program: the ones with at least one candidate. */
export function applicableKinds(program, kinds = REWRITE_KINDS) {
  const tagged = tagSites(program);
  return kinds.filter((kind) => REWRITES[kind].candidates(tagged, () => "eq0").length > 0);
}

/**
 * Applies one rewrite kind to a copy of `program`.
 * `choose(candidates)` picks which candidates to apply (default all). Returns `{ program, kind, applied }` (the rewritten tree and
 * what changed), or `null` when nothing applied.
 */
export function rewriteProgram(program, kind, { choose = (all) => all } = {}) {
  const rewrite = REWRITES[kind];
  if (!rewrite) throw new Error(`unknown rewrite ${kind}`);
  const variant = tagSites(program);
  let counter = 0;
  const fresh = () => { counter += 1; return `eq${counter}`; };
  const candidates = rewrite.candidates(variant, fresh);
  const chosen = choose(candidates);
  const applied = [];
  for (const candidate of chosen) if (candidate.apply()) applied.push(candidate.describe);
  return applied.length ? { program: variant, kind, applied } : null;
}
