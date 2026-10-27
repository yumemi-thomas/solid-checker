// Renders a program tree as an `App.tsx` and a table of its sites.
//
// A site is a `read` or `write` node flagged `site: true`. The printer plants an
// invisible marker before each one, joins the file, then reads the marker's line
// and strips it, so a site's line is the line its call starts on, whatever the
// nesting. Names, spacing and statement order are functions of the tree alone, so
// two equal trees print byte-equal files (the dedupe and shrink caches rely on it).
//
// Two texts come out of one tree. `text` is clean: what the checker reads and what an owner
// reads in a repro. `runText` is its instrumented twin (the hit ledger, hits.mjs): the same
// file with `(__h(N), expr)` counters on the payload sites and on every other emitted call
// (the glue), `{ __h(N);` at every function entry (a statement that is itself a counted call
// is counted by `__h(N); <statement>`), and the counter's definition appended after
// the last line, so no line moves. The type check, the fast path and Chrome run `runText`.
import { INSTRUMENT_PRELUDE } from "./hits.mjs";
import { isSite, paramTsType, signalTable, walk } from "./model.mjs";

const MARK_OPEN = "\u0001";
const MARK_CLOSE = "\u0002";
const INDENT = "  ";

/** Node kinds whose print is a call the runtime executes: each is an auto-site (glue) in the twin. */
export const GLUE_KINDS = ["call", "set", "iife", "iter", "prim", "getOwner"];

/**
 * @param {object} program
 * @param {{ instrument?: boolean }} [options] `instrument: false` skips the twin (`runText` is `text`, the ledger empty)
 * @returns {{ text: string, runText: string, sites: object[], glue: object[], ledger: object[] }}
 *   `sites`: the payload sites, each with its `hit` id in the twin.
 *   `glue`: one auto-site per emitted call, `{ id: "aN", hit, kind, line, path }`.
 *   `ledger`: every counter `{ hit, kind: site|auto|entry, ref, node, line, path, enclosing, inDefault }`;
 *   `enclosing` is the hit id of the nearest enclosing function entry (null for a component's own entry);
 *   `inDefault` marks a counter inside a parameter default (run only when the caller passes no argument).
 */
export function printProgram(program, { instrument = true } = {}) {
  const clean = render(program, false);
  if (!instrument) return { text: clean.text, runText: clean.text, sites: clean.sites, glue: [], ledger: [] };
  const run = render(program, true);
  const sites = clean.sites.map((site, index) => ({ ...site, hit: run.siteHits[index] }));
  const bySiteHit = new Map(sites.map((site) => [site.hit, site.id]));
  const glue = [];
  const ledger = run.ledger.map((row) => {
    const base = { hit: row.hit, kind: row.kind, node: row.node, line: row.line, path: row.path, enclosing: row.enclosing, inDefault: row.inDefault };
    if (row.kind === "site") return { ...base, ref: bySiteHit.get(row.hit) };
    if (row.kind === "auto") {
      const auto = { id: `a${glue.length + 1}`, hit: row.hit, kind: row.node, line: row.line, path: row.path };
      glue.push(auto);
      return { ...base, ref: auto.id };
    }
    return { ...base, ref: `e${row.hit}` };
  });
  return { text: clean.text, runText: `${run.text}${INSTRUMENT_PRELUDE}`, sites, glue, ledger };
}

function render(program, instrument) {
  // Site numbering follows the tree walk, which is the order sites appear in the file.
  const siteIndex = new Map();
  const sitePaths = [];
  const pathOf = new Map();
  // A call that is the callee (or the receiver of the member that is the callee) of another call is a link in a chain
  // (`m1()()`, `m1().f()`): only the chain's top is counted, so the checker still sees the chain it would see unmarked.
  const uncounted = new Set();
  // A call in argument position (`h2(h1(() => ...))`, `setK1(m1())`) is not counted on its own: a comma expression there hides
  // the exact closure or value the checker follows through the callee (the regression found `no-write-in-pure-computation`
  // turn uncertifiable). The statement that holds it is counted, on the same line.
  for (const { node, path } of walk(program)) {
    pathOf.set(node, path);
    if (node.k === "call") for (const arg of node.args) if (GLUE_KINDS.includes(arg.k)) uncounted.add(arg);
    if (node.k === "set" && GLUE_KINDS.includes(node.e.k)) uncounted.add(node.e);
    if (node.k === "call") {
      for (let callee = node.callee; callee; callee = callee.k === "member" ? callee.obj : null) {
        if (callee.k === "call" || callee.k === "prim" || callee.k === "iife") uncounted.add(callee);
        if (callee.k !== "member") break;
      }
    }
    if (isSite(node)) {
      siteIndex.set(node, sitePaths.length);
      sitePaths.push({ node, path });
    }
  }
  const used = new Set(["createSignal"]);
  const signals = signalTable(program);
  const helpers = new Map((program.helpers ?? []).map((helper) => [helper.name, helper]));

  // The hit ledger: one id per counter, allocated in print order (a function of the tree alone).
  const ledger = [];
  const siteHits = [];
  const scopes = [{ entry: null, inDefault: false }];
  const allocate = (kind, node, path) => {
    const hit = ledger.length + 1;
    const scope = scopes.at(-1);
    ledger.push({ hit, kind, node, path, enclosing: scope.entry, inDefault: scope.inDefault });
    return hit;
  };
  const within = (scope, print) => {
    scopes.push(scope);
    try { return print(); } finally { scopes.pop(); }
  };
  const entryFor = (node, kind) => (instrument ? allocate("entry", kind, pathOf.get(node)) : null);
  const counted = (node) => siteIndex.has(node) || GLUE_KINDS.includes(node.k);
  const counterFor = (node) => {
    if (!siteIndex.has(node)) return allocate("auto", node.k, pathOf.get(node));
    const hit = allocate("site", node.k, pathOf.get(node));
    siteHits[siteIndex.get(node)] = hit;
    return hit;
  };
  // A statement whose expression is itself a counted call is counted by a statement of its own, `__h(N); <statement>` on the
  // same line: the expression the checker reads stays exactly as it would be unmarked (a comma expression is not the same
  // to it: `(__h(8), m1()())` as a statement hides the read the checker reports on `m1()()`).
  const prefixed = new Set();
  const counting = (e, build) => {
    if (!instrument || !counted(e) || uncounted.has(e)) return build();
    const hit = counterFor(e);
    prefixed.add(e);
    return `__h(${hit}); ${build()}`;
  };
  /** `(__h(N), <print>)` around an emitted call; the id is allocated before its children's. */
  const glue = (node, build) => {
    if (!instrument || uncounted.has(node) || prefixed.has(node)) return build();
    const hit = allocate("auto", node.k, pathOf.get(node));
    return `(__h(${hit}), ${build()})`;
  };

  const mark = (node) => (siteIndex.has(node) ? `${MARK_OPEN}${siteIndex.get(node)}${MARK_CLOSE}` : "");
  const siteCall = (node, text) => {
    if (!instrument || !siteIndex.has(node) || prefixed.has(node)) return `${mark(node)}${text}`;
    const hit = counterFor(node);
    return `${mark(node)}(__h(${hit}), ${text})`;
  };
  const ind = (depth) => INDENT.repeat(depth);

  function params(f, depth) {
    return f.params.map((p, index) => {
      const name = `p${index}`;
      const value = expr(p.default, depth);
      return p.destructured ? `{ ${name} = ${value} }: { ${name}?: ${paramTsType(p)} } = {}` : `${name}: ${paramTsType(p)} = ${value}`;
    }).join(", ");
  }
  /** The parameter list and body of a function node. Defaults are evaluated at the call, before the entry counter. */
  function fnParts(f, depth, header) {
    const entry = entryFor(f, "fn");
    const head = header ?? `(${within({ entry, inDefault: true }, () => params(f, depth))})`;
    return { head, body: block(f.body, depth, entry) };
  }
  function fnText(f, depth, { header } = {}) {
    const { head, body } = fnParts(f, depth, header);
    return `${f.async ? "async " : ""}${head} => ${body}`;
  }
  /** A block; `entry` (a hit id) plants the function-entry counter on the opening line and scopes the statements under it. */
  function block(stmts, depth, entry = null, lead = []) {
    const hit = entry === null ? "" : `__h(${entry});`;
    if (!stmts.length && !lead.length) return hit ? `{ ${hit} }` : "{}";
    const inner = () => [...lead.map((line) => `${ind(depth + 1)}${line}`), ...stmts.map((stmt) => `${ind(depth + 1)}${stmtText(stmt, depth + 1)}`)].join("\n");
    const text = entry === null ? inner() : within({ entry, inDefault: false }, inner);
    return `{${hit ? ` ${hit}` : ""}\n${text}\n${ind(depth)}}`;
  }
  function stmtText(stmt, depth) {
    switch (stmt.k) {
      case "expr": return counting(stmt.e, () => `${expr(stmt.e, depth)};`);
      case "decl":
        if (stmt.e.k === "fn" && stmt.e.form === "function") {
          const { head, body } = fnParts(stmt.e, depth, null);
          return `${stmt.e.async ? "async " : ""}function ${stmt.name}${head} ${body}`;
        }
        return counting(stmt.e, () => `${stmt.kw ?? "const"} ${stmt.name} = ${expr(stmt.e, depth)};`);
      case "return": return counting(stmt.e, () => `return ${expr(stmt.e, depth)};`);
      case "await": return "await Promise.resolve();";
      case "try": return `try ${block(stmt.body, depth)} finally ${block(stmt.fin, depth)}`;
      case "contain": return containText(stmt, depth);
      default: throw new Error(`cannot print statement ${stmt.k}`);
    }
  }
  function expr(e, depth) {
    switch (e.k) {
      case "num": return String(e.n);
      case "read": return siteCall(e, `${e.sig}()`);
      case "write": return siteCall(e, writeText(e));
      case "own": return siteCall(e, ownText(e, depth));
      case "ref": return e.name;
      case "member": return `${expr(e.obj, depth)}.${e.key}`;
      case "obj": return e.fields.length ? `{ ${e.fields.map((f) => {
        if (f.e.k === "ref" && f.e.name === f.key) return f.key;
        if (f.e.k === "fn" && f.e.form === "method") { const { head, body } = fnParts(f.e, depth, null); return `${f.e.async ? "async " : ""}${f.key}${head} ${body}`; }
        return `${f.key}: ${expr(f.e, depth)}`;
      }).join(", ")} }` : "{}";
      case "fn": return fnText(e, depth);
      case "call": return glue(e, () => callText(e, depth));
      case "nul": return "null";
      case "jsx": return "<span />";
      case "set": return glue(e, () => `${signals.get(e.sig).setter}(${expr(e.e, depth)})`);
      case "getOwner": used.add("getOwner"); return glue(e, () => "getOwner()");
      case "flush": used.add("flush"); return "flush()";
      case "iife": return glue(e, () => `(${expr(e.fn, depth)})()`);
      case "iter": return glue(e, () => (e.method === "map" ? `[1].map(${expr(e.fn, depth)})[0]` : `[1].forEach(${expr(e.fn, depth)})`));
      case "prim": return glue(e, () => primText(e, depth));
      default: throw new Error(`cannot print expression ${e.k}`);
    }
  }
  /** A callee that is a name or a member chain of names: the forms a `via` can rewrite without changing what is called. */
  const plainCallee = (callee) => callee.k === "ref" || (callee.k === "member" && plainCallee(callee.obj));
  function callText(e, depth) {
    const callee = e.callee.k === "fn" || e.callee.k === "prim" ? `(${expr(e.callee, depth)})` : expr(e.callee, depth);
    const args = e.args.map((a) => expr(a, depth)).join(", ");
    if (e.via && plainCallee(e.callee)) {
      switch (e.via) {
        case "optional": return `${callee}?.(${args})`;
        case "as": return `(${callee} as typeof ${callee})(${args})`;
        case "comma": return `(void 0, ${callee})(${args})`;
        case "and": return `(true && ${callee})(${args})`;
        case "paren": return `(${callee})(${args})`;
        default: throw new Error(`unknown call form ${e.via}`);
      }
    }
    return `${callee}(${args})`;
  }
  /** An effect's apply: a function literal with the value as its parameter, or a declared name. */
  const applyText = (apply, depth) => (apply.k === "ref" ? apply.name : fnText(apply, depth, { header: "(v)" }));

  /** A container (model.mjs CONTAINER_KINDS): statements written where they run elsewhere, later, or not at all. */
  function containText(c, depth) {
    const name = c.kind.startsWith("comp") ? `Child${c.id}` : c.kind.startsWith("class") ? `K${c.id}` : `dead${c.id}`;
    const open = () => entryFor(c, "contain");
    const asyncKw = c.async ? "async " : "";
    const direct = c.body.length === 1 && c.body[0].k === "expr";
    const funcDecl = (at) => `${asyncKw}function ${name}() ${block(c.body, at, open())}`;
    const line = (text) => `\n${ind(depth)}${text}`;
    switch (c.kind) {
      case "comp-never": return funcDecl(depth);
      case "comp-rendered": return `${funcDecl(depth)}${line(`<${name} />;`)}`;
      case "comp-slot": return `${funcDecl(depth)}${line(`({ slot: (c: () => unknown) => c() }).slot(${name});`)}`;
      case "comp-prop": return `${funcDecl(depth)}${line(`props.slot?.(${name});`)}`;
      case "comp-returned": {
        const inner = funcDecl(depth + 1);
        return `(() => {\n${ind(depth + 1)}${inner}\n${ind(depth + 1)}return ${name};\n${ind(depth)}})()${c.use === "call" ? "()" : ""};`;
      }
      case "class-field": case "class-static": {
        const init = direct ? expr(c.body[0].e, depth + 1) : `(() => ${block(c.body, depth + 1, open())})()`;
        const member = `${c.kind === "class-static" ? "static " : ""}f = ${init};`;
        return `class ${name} {\n${ind(depth + 1)}${member}\n${ind(depth)}}${c.kind === "class-field" && c.use === "new" ? line(`new ${name}();`) : ""}`;
      }
      case "class-sblock": return `class ${name} {\n${ind(depth + 1)}static ${block(c.body, depth + 1, open())}\n${ind(depth)}}`;
      case "class-method": return `class ${name} {\n${ind(depth + 1)}${asyncKw}m() ${block(c.body, depth + 1, open())}\n${ind(depth)}}${c.use === "call" ? line(`new ${name}().m();`) : c.use === "new" ? line(`new ${name}();`) : ""}`;
      case "class-ctor": return `class ${name} {\n${ind(depth + 1)}constructor() ${block(c.body, depth + 1, open())}\n${ind(depth)}}${c.use === "new" ? line(`new ${name}();`) : ""}`;
      case "dead-if": return `if (false) ${block(c.body, depth, open())}`;
      case "dead-after-return": return `(${asyncKw}() => ${block(c.body, depth, open(), ["return;"])})();`;
      case "dead-fn": return funcDecl(depth);
      case "and-true": case "and-false": {
        const cond = c.kind === "and-true" ? "true" : "false";
        return direct ? `${cond} && ${expr(c.body[0].e, depth)};` : `${cond} && (() => ${block(c.body, depth, open())})();`;
      }
      default: throw new Error(`cannot print container ${c.kind}`);
    }
  }

  /** A write by its target's kind (model.mjs storeSignal, memoSignal, actionSignal); a plain signal is `setX(value)`. */
  function writeText(e) {
    const sig = signals.get(e.sig);
    switch (sig.kind) {
      case "store": used.add("createStore"); return `${sig.setter}((s) => { s.v = ${e.value}; })`;
      case "memo": used.add("refresh"); return `refresh(${sig.name})`;
      case "action": return `${sig.name}()`;
      default: return `${sig.setter}(${e.value})`;
    }
  }
  function ownText(e, depth) {
    switch (e.op) {
      case "cleanup": used.add("onCleanup"); return `onCleanup(${expr(e.fn, depth)})`;
      case "settled": used.add("onSettled"); return `onSettled(${expr(e.fn, depth)})`;
      case "effect": used.add("createEffect"); return `createEffect(${expr(e.fn, depth)}, ${applyText(e.apply, depth)})`;
      case "context": used.add("useContext"); return "useContext(Ctx)";
      default: throw new Error(`cannot print owner op ${e.op}`);
    }
  }
  function primText(e, depth) {
    const f = e.name === "rootd" ? null : expr(e.fn, depth);
    switch (e.name) {
      case "memo": used.add("createMemo"); return `createMemo(${f})`;
      case "effect": used.add("createEffect"); return `createEffect(${f}, ${applyText(e.apply, depth)})`;
      case "untrack": used.add("untrack"); return `untrack(${f})`;
      case "root": used.add("createRoot"); return `createRoot(${f})`;
      case "owner": {
        used.add("runWithOwner");
        if (e.ownerArg === "create") used.add("createOwner");
        return `runWithOwner(${e.ownerArg === "owner" ? "owner" : e.ownerArg === "create" ? "createOwner()" : "null"}, ${f})`;
      }
      case "rootd": used.add("createRoot"); return `createRoot(${fnText(e.fn, depth, { header: "(dispose)" })})`;
      case "micro": return `queueMicrotask(${f})`;
      case "settled": used.add("onSettled"); return `onSettled(${f})`;
      case "cleanup": used.add("onCleanup"); return `onCleanup(${f})`;
      case "timer": return `setTimeout(${f}, 0)`;
      case "then": return `Promise.resolve().then(${f})`;
      default: throw new Error(`cannot print prim ${e.name}`);
    }
  }

  function viewText(item, depth) {
    switch (item.k) {
      case "probe": return [`${ind(depth)}<output data-probe="${item.sig}">{${signals.get(item.sig)?.kind === "store" ? `${item.sig}.v` : `${item.sig}()`}}</output>`];
      case "button": {
        const lines = [`${ind(depth)}<button data-act="${item.id}" onClick={() => ${block(item.handler, depth, entryFor(item, "handler"))}}>go</button>`];
        return lines;
      }
      case "child": return [`${ind(depth)}<p data-view="${item.id}">{${expr(item.e, depth)}}</p>`];
      case "attr": return [`${ind(depth)}<p data-attr={${expr(item.e, depth)}} data-view="${item.id}">x</p>`];
      case "show": {
        used.add("Show");
        return [`${ind(depth)}<Show when={${expr(item.when, depth)}}>`, ...item.children.flatMap((c) => viewText(c, depth + 1)), `${ind(depth)}</Show>`];
      }
      case "for": {
        // One row: a selector must match exactly one element (the scenario's click and wait steps are strict).
        used.add("For");
        return [`${ind(depth)}<For each={[1]}>`, `${ind(depth + 1)}{() => (`, `${ind(depth + 2)}<li>`,
          ...item.children.flatMap((c) => viewText(c, depth + 3)), `${ind(depth + 2)}</li>`, `${ind(depth + 1)})}`, `${ind(depth)}</For>`];
      }
      default: throw new Error(`cannot print view ${item.k}`);
    }
  }

  const signalLine = (sig) => {
    switch (sig.kind) {
      case "store": used.add("createStore"); return `const [${sig.name}, ${sig.setter}] = createStore({ v: ${sig.init} });`;
      case "memo": used.add("createMemo"); return `const ${sig.name} = createMemo(() => 1);`;
      case "action": used.add("action"); return `const ${sig.name} = action(function* () { ${signals.get(sig.target).setter}(${sig.value}); });`;
      default: return `const [${sig.name}, ${sig.setter}] = createSignal(${sig.init});`;
    }
  };
  const out = [];
  // `useContext(Ctx)`: one module-level context with a default value (a read with no provider is then not an error; no owner is).
  const usesContext = [...walk(program)].some(({ node }) => node.k === "own" && node.op === "context");
  if (usesContext) { used.add("createContext"); out.push("const Ctx = createContext(7);", ""); }
  const moduleSignals = program.moduleSignals ?? [];
  moduleSignals.forEach((sig) => out.push(signalLine(sig)));
  if (moduleSignals.length) out.push("");
  for (const helper of helpers.values()) {
    out.push(...helperText(helper), "");
  }
  for (const component of program.components) {
    const entry = entryFor(component, "component");
    const usesProps = [...walk(component)].some(({ node }) => node.k === "contain" && node.kind === "comp-prop");
    out.push(`export function ${component.name}(${usesProps ? "props: { slot?: (c: () => unknown) => unknown }" : ""}) {${entry === null ? "" : ` __h(${entry});`}`);
    within({ entry, inDefault: false }, () => {
      for (const sig of component.signals) out.push(`${INDENT}${signalLine(sig)}`);
      for (const stmt of component.body) out.push(`${INDENT}${stmtText(stmt, 1)}`);
      out.push(`${INDENT}return (`, `${INDENT}${INDENT}<div>`);
      for (const item of component.view) out.push(...viewText(item, 3));
    });
    out.push(`${INDENT}${INDENT}</div>`, `${INDENT});`, "}", "");
  }
  const importNames = [...used].sort();
  const header = `import { ${importNames.join(", ")} } from "solid-js";\n\n`;
  // Printing the body filled `used`; the header goes first, so line numbers shift by its height.
  const body = out.join("\n");
  const full = `${header}${body}`.replace(/\n+$/, "\n");

  const lines = full.split("\n");
  const siteLines = new Map();
  const markPattern = new RegExp(`${MARK_OPEN}(\\d+)${MARK_CLOSE}`, "g");
  const hitLines = new Map();
  lines.forEach((line, row) => {
    for (const match of line.matchAll(markPattern)) siteLines.set(Number(match[1]), row + 1);
    if (instrument) for (const match of line.matchAll(/__h\((\d+)\)/g)) hitLines.set(Number(match[1]), row + 1);
  });
  const text = full.replace(markPattern, "");
  const sites = sitePaths.map(({ node, path }, index) => ({
    id: `s${index + 1}`, kind: node.k, sig: node.sig, op: node.k === "own" ? node.op : undefined, value: node.k === "write" ? node.value : undefined,
    line: siteLines.get(index), path, witness: node.witness ?? null,
  }));
  for (const row of ledger) row.line = hitLines.get(row.hit);
  return { text, sites, siteHits, ledger };

  function helperText(helper) {
    switch (helper.form) {
      case "call": return [`function ${helper.name}(cb: () => unknown) {`, `${INDENT}cb();`, `${INDENT}return 1;`, "}"];
      case "defaultCall": return [`function ${helper.name}(cb: () => unknown, x: unknown = cb()) {`, `${INDENT}return x;`, "}"];
      case "returnClosure": return [`function ${helper.name}(cb: () => unknown) {`, `${INDENT}const g = (x: unknown = cb()) => x;`, `${INDENT}return g;`, "}"];
      case "returnCb": return [`function ${helper.name}(cb: () => unknown) {`, `${INDENT}return cb;`, "}"];
      case "storeCb": return [`function ${helper.name}(cb: () => unknown) {`, `${INDENT}return { cb };`, "}"];
      case "timerCb": return [`function ${helper.name}(cb: () => unknown) {`, `${INDENT}setTimeout(cb, 0);`, `${INDENT}return 1;`, "}"];
      default: throw new Error(`unknown helper form ${helper.form}`);
    }
  }
}

/** The ordered click plan: every button, in the order the file declares them. */
export function drivingPlan(program) {
  const clicks = [];
  for (const { node } of walk(program)) if (node.k === "button") clicks.push(node.id);
  return { clicks, settleMs: 400 };
}
