// Renders a program tree as an `App.tsx` and a table of its sites.
//
// A site is a `read` or `write` node flagged `site: true`. The printer plants an
// invisible marker before each one, joins the file, then reads the marker's line
// and strips it, so a site's line is the line its call starts on, whatever the
// nesting. Names, spacing and statement order are functions of the tree alone, so
// two equal trees print byte-equal files (the dedupe and shrink caches rely on it).
import { paramTsType, signalTable, walk } from "./model.mjs";

const MARK_OPEN = "\u0001";
const MARK_CLOSE = "\u0002";
const INDENT = "  ";

export function printProgram(program) {
  // Site numbering follows the tree walk, which is the order sites appear in the file.
  const siteIndex = new Map();
  const sitePaths = [];
  for (const { node, path } of walk(program)) {
    if ((node.k === "read" || node.k === "write") && node.site) {
      siteIndex.set(node, sitePaths.length);
      sitePaths.push({ node, path });
    }
  }
  const used = new Set(["createSignal"]);
  const signals = signalTable(program);
  const helpers = new Map((program.helpers ?? []).map((helper) => [helper.name, helper]));

  const mark = (node) => (siteIndex.has(node) ? `${MARK_OPEN}${siteIndex.get(node)}${MARK_CLOSE}` : "");
  const ind = (depth) => INDENT.repeat(depth);

  function params(f, depth) {
    return f.params.map((p, index) => {
      const name = `p${index}`;
      const value = expr(p.default, depth);
      return p.destructured ? `{ ${name} = ${value} }: { ${name}?: ${paramTsType(p)} } = {}` : `${name}: ${paramTsType(p)} = ${value}`;
    }).join(", ");
  }
  function fnText(f, depth, { header } = {}) {
    const head = header ?? `(${params(f, depth)})`;
    return `${f.async ? "async " : ""}${head} => ${block(f.body, depth)}`;
  }
  function block(stmts, depth) {
    if (!stmts.length) return "{}";
    return `{\n${stmts.map((stmt) => `${ind(depth + 1)}${stmtText(stmt, depth + 1)}`).join("\n")}\n${ind(depth)}}`;
  }
  function stmtText(stmt, depth) {
    switch (stmt.k) {
      case "expr": return `${expr(stmt.e, depth)};`;
      case "decl":
        if (stmt.e.k === "fn" && stmt.e.form === "function") return `${stmt.e.async ? "async " : ""}function ${stmt.name}(${params(stmt.e, depth)}) ${block(stmt.e.body, depth)}`;
        return `const ${stmt.name} = ${expr(stmt.e, depth)};`;
      case "return": return `return ${expr(stmt.e, depth)};`;
      case "await": return "await Promise.resolve();";
      default: throw new Error(`cannot print statement ${stmt.k}`);
    }
  }
  function expr(e, depth) {
    switch (e.k) {
      case "num": return String(e.n);
      case "read": return `${mark(e)}${e.sig}()`;
      case "write": return `${mark(e)}${signals.get(e.sig).setter}(${e.value})`;
      case "ref": return e.name;
      case "member": return `${expr(e.obj, depth)}.${e.key}`;
      case "obj": return e.fields.length ? `{ ${e.fields.map((f) => (f.e.k === "ref" && f.e.name === f.key ? f.key : `${f.key}: ${expr(f.e, depth)}`)).join(", ")} }` : "{}";
      case "fn": return fnText(e, depth);
      case "call": return `${e.callee.k === "fn" || e.callee.k === "prim" ? `(${expr(e.callee, depth)})` : expr(e.callee, depth)}(${e.args.map((a) => expr(a, depth)).join(", ")})`;
      case "set": return `${signals.get(e.sig).setter}(${expr(e.e, depth)})`;
      case "getOwner": used.add("getOwner"); return "getOwner()";
      case "iife": return `(${expr(e.fn, depth)})()`;
      case "iter": return e.method === "map" ? `[1].map(${expr(e.fn, depth)})[0]` : `[1].forEach(${expr(e.fn, depth)})`;
      case "prim": return primText(e, depth);
      default: throw new Error(`cannot print expression ${e.k}`);
    }
  }
  function primText(e, depth) {
    const f = expr(e.fn, depth);
    switch (e.name) {
      case "memo": used.add("createMemo"); return `createMemo(${f})`;
      case "effect": used.add("createEffect"); return `createEffect(${f}, ${fnText(e.apply, depth, { header: "(v)" })})`;
      case "untrack": used.add("untrack"); return `untrack(${f})`;
      case "root": used.add("createRoot"); return `createRoot(${f})`;
      case "owner": used.add("runWithOwner"); return `runWithOwner(${e.ownerArg === "owner" ? "owner" : "null"}, ${f})`;
      case "settled": used.add("onSettled"); return `onSettled(${f})`;
      case "cleanup": used.add("onCleanup"); return `onCleanup(${f})`;
      case "timer": return `setTimeout(${f}, 0)`;
      case "then": return `Promise.resolve().then(${f})`;
      default: throw new Error(`cannot print prim ${e.name}`);
    }
  }

  function viewText(item, depth) {
    switch (item.k) {
      case "probe": return [`${ind(depth)}<output data-probe="${item.sig}">{${item.sig}()}</output>`];
      case "button": {
        const lines = [`${ind(depth)}<button data-act="${item.id}" onClick={() => ${block(item.handler, depth)}}>go</button>`];
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

  const signalLine = (sig) => `const [${sig.name}, ${sig.setter}] = createSignal(${sig.init});`;
  const out = [];
  const moduleSignals = program.moduleSignals ?? [];
  moduleSignals.forEach((sig) => out.push(signalLine(sig)));
  if (moduleSignals.length) out.push("");
  for (const helper of helpers.values()) {
    out.push(...helperText(helper), "");
  }
  for (const component of program.components) {
    out.push(`export function ${component.name}() {`);
    for (const sig of component.signals) out.push(`${INDENT}${signalLine(sig)}`);
    for (const stmt of component.body) out.push(`${INDENT}${stmtText(stmt, 1)}`);
    out.push(`${INDENT}return (`, `${INDENT}${INDENT}<div>`);
    for (const item of component.view) out.push(...viewText(item, 3));
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
  lines.forEach((line, row) => {
    for (const match of line.matchAll(markPattern)) siteLines.set(Number(match[1]), row + 1);
  });
  const text = full.replace(markPattern, "");
  const sites = sitePaths.map(({ node, path }, index) => ({
    id: `s${index + 1}`, kind: node.k, sig: node.sig, value: node.k === "write" ? node.value : undefined,
    line: siteLines.get(index), path, witness: node.witness ?? null,
  }));
  return { text, sites };

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
