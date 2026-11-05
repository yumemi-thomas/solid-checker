// The flip's read track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_READ_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipReadContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
//   FLIP_READ_LEGAL     { [code]: [context, ...] }, appended to that code's recallLegal
//   FLIP_READ_VARIANTS  { [code]: { [variant]: { kind, contexts } } }, a source of that kind, in those contexts, added to the code's recall variants
import { apiSignal, button, call, child, decl, domExpr, expr, fn, iife, jsxExpr, mounted, num, prim, ref, ret } from "./model.mjs";

// ADR 0654: `dynamic(source, options)` as each variant writes it: every one throws in dev, by the source's shape or the options'.
export const STATIC_SOURCES = {
  "src-block": '() => { return Promise.resolve("p" as const); }, { static: true }',
  "src-then": '() => Promise.resolve(1).then(() => "p" as const), { static: true }',
  "src-any": '(() => Promise.resolve("p" as const)) as () => any, { static: true }',
  "src-cond": '() => (Math.random() >= 0 ? Promise.resolve("p" as const) : ("p" as const)), { static: true }',
  "src-options": '() => Promise.resolve("p" as const), ({ static: true }) as { static: boolean }',
};

// ADR 0655: a `sync: true` computation whose return no type settles. Each returns a Promise when it runs.
export const SYNC_COMPUTES = {
  "cmp-any": '((() => Promise.resolve(1)) as () => any), { sync: true }',
  "cmp-unknown": '((() => Promise.resolve(1)) as () => unknown), { sync: true }',
  "cmp-cond": '() => (Math.random() >= 0 ? Promise.resolve(1) : 1), { sync: true }',
  "cmp-options": 'async () => 1, ({ sync: true }) as { sync: boolean }',
};

// ADR 0657: a destructure no exact root reaches. Each is a Get of the store at setup.
export const STORE_DESTRUCTURES = {
  "store-dstr-assign": { reads: true, text: (o) => `let v = 0; ({ v } = ${o}); void v;` },
  "store-dstr-array": { reads: true, text: (o) => `const [{ v }] = [${o}]; void v;` },
  "store-dstr-for-of": { reads: true, text: (o) => `for (const { v } of [${o}]) void v;` },
  "store-dstr-merge": { reads: true, imports: ["merge"], text: (o) => `const { v } = merge(${o}, { w: 1 }); void v;` },
};

export const FLIP_READ_CONTEXTS = {
  LOADING_ON_ACCESSOR: ["on-any", "on-unknown", "on-spread", "on-json", "on-prop", "on-call"],
  // ADR 0662, ADR 0668: a `<Dynamic>` body has no strict window and still warns with no boundary; a `<Portal>` is a plain synchronous mount.
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["dyn-jsx-body", "portal-jsx", "portal-dyn"],
  PENDING_ASYNC_UNTRACKED_READ: ["portal-body", "dyn-jsx-mixed"],
  // ADR 0669: a memo's promise chain with a handler handed by name.
  UNTRACKED_READ_AFTER_AWAIT: ["then-named"],
  // ADR 0679: a named function holding a window-inheriting call, run directly from the body.
  STRICT_READ_UNTRACKED: ["root-named-twice", "root-named-foreach"],
};

// ADR 0661, ADR 0668: quiet in Chrome. A mount after `render()` returned has no boundary check, and the source settled; a `<Dynamic>` body has
// no window, so a pending read retries. Under `<Loading>`: no placeless boundary warning leaves the program unjudged.
export const FLIP_READ_LEGAL = {
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["show-false-later", "ternary-later", "and-later", "dyn-jsx-later"],
  PENDING_ASYNC_UNTRACKED_READ: ["dyn-jsx-body-l", "dyn-fn-body-l"],
  // ADR 0678: `render(Local, el)` calls `Local` under a root, with no strict window. ADR 0677: a client build never runs a helper only `isServer` reaches.
  STRICT_READ_UNTRACKED: ["mount-named", "arm-helper", "arm-helper-chain", "arm-tag", "root-named-timer", "root-named-then", "root-two-handlers", "apply-cleanup-mount", "subclass-late-foreach"],
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["arm-helper", "arm-helper-chain", "arm-tag"],
  STORE_WRITE_OUTSIDE_SETTER: ["arm-helper"],
};

// ADR 0665: a source no type proves sync or async. Each is pending at the read; the build before took it for a settled signal.
export const FLIP_READ_VARIANTS = {
  PENDING_ASYNC_UNTRACKED_READ: { "open-any": { kind: "open-any", contexts: ["body", "root"] }, "open-notready": { kind: "open-notready", contexts: ["body", "root"] } },
};
export const openSignal = (name, kind) => apiSignal(name, 0, kind, "src");

// A value of the boundary's own, beside the site: the context is the way `on` is handed. The boundary is built in the body: a view child must be a number.
const loadingOnForms = (t) => ({
  "on-any": { body: [decl("f", domExpr(`${t} as any`))], tag: "<Loading on={f} fallback={0}><i /></Loading>" },
  "on-unknown": { body: [decl("f", domExpr(`${t} as unknown`))], tag: "<Loading on={f} fallback={0}><i /></Loading>" },
  "on-spread": { body: [decl("p", domExpr(`{ on: ${t} }`))], tag: "<Loading {...p} fallback={0}><i /></Loading>" },
  "on-json": { body: [decl("o", domExpr(`JSON.parse("{}")`))], tag: "<Loading on={o.f} fallback={0}><i /></Loading>", first: `o.f = ${t};` },
  "on-prop": { body: [decl("B", domExpr("(props: { on: unknown }) => <Loading on={props.on} fallback={0}><i /></Loading>"))], tag: `<B on={${t}} />` },
  "on-call": { body: [], tag: `Loading({ on: ${t}, fallback: 0, children: <i /> }) as any` },
});

// The component that holds the site, and how a parent mounts it.
const holderForms = (k) => ({
  "dyn-jsx-body": { read: "body", mount: `<Dynamic component={${k}} />` },
  "dyn-jsx-mixed": { read: "body", mount: `<span><Dynamic component={${k}} /><${k} /></span>` },
  "dyn-jsx-body-l": { read: "body", mount: `<Loading fallback={0}><Dynamic component={${k}} /></Loading>` },
  "dyn-fn-body-l": { read: "body", mount: `<Loading fallback={0}><D${k} /></Loading>`, prelude: `const D${k} = dynamic(() => ${k});` },
  "portal-body": { read: "body", mount: `<Portal><${k} /></Portal>` },
  "portal-jsx": { read: "jsx", mount: `<Portal><${k} /></Portal>` },
  "portal-dyn": { read: "body", mount: `<Portal><Dynamic component={${k}} /></Portal>` },
  "show-false-later": { read: "jsx", later: (on) => `<Show when={${on}[0]()}><${k} /></Show>` },
  "ternary-later": { read: "jsx", later: (on) => `{${on}[0]() ? <${k} /> : null}` },
  "and-later": { read: "jsx", later: (on) => `{${on}[0]() && <${k} />}` },
  "dyn-jsx-later": { read: "jsx", later: (on) => `<Show when={${on}[0]()}><Dynamic component={${k}} /></Show>` },
});

export function flipReadContext(context, stmts, kit) {
  if (context === "mount-named") {
    const h = kit.name("f");
    return { hoisted: [decl(h, fn([...stmts, ret(domExpr("<i />"))])), expr(domExpr(`render(${h}, document.createElement("div"));`, ["render"]))], body: [], view: [] };
  }
  if (context === "arm-helper" || context === "arm-helper-chain") {
    const [h, g] = [kit.name("f"), kit.name("g")];
    const held = context === "arm-helper" ? [decl(h, fn(stmts))] : [decl(g, fn(stmts)), decl(h, fn([expr(call(ref(g)))]))];
    return { body: [...held, expr(domExpr(`if (isServer) ${h}();`, ["isServer"]))], view: [] };
  }
  if (context.startsWith("root-named-") || context === "root-two-handlers") {
    const h = kit.name("f");
    const held = decl(h, fn([expr(prim("root", fn(stmts)))]));
    switch (context) {
      case "root-named-timer": return { body: [held, expr(domExpr(`setTimeout(${h}, 0)`))], view: [] };
      case "root-named-then": return { body: [held, expr(domExpr(`Promise.resolve().then(${h})`))], view: [] };
      case "root-named-twice": return { body: [held, expr(call(ref(h))), expr(call(ref(h)))], view: [] };
      case "root-named-foreach": return { body: [held, expr(domExpr(`[1].forEach(${h})`))], view: [] };
      default: return { body: [held], view: [button(kit.name("b"), [expr(call(ref(h)))]), button(kit.name("b"), [expr(call(ref(h)))])] };
    }
  }
  // ADR 0681: `createEffect`'s apply has no owner, so this cleanup never registers and `K` never mounts.
  if (context === "apply-cleanup-mount") {
    const k = kit.name("K");
    const hold = `onCleanup(() => { render(() => <${k} />, document.createElement("div")); })`;
    return { body: [decl(k, fn([...stmts, ret(domExpr("<i />"))])), expr(domExpr(`createEffect(() => 1, () => { ${hold}; })`, ["render"], ["createEffect", "onCleanup"]))], view: [] };
  }
  // ADR 0683: the override runs the callback on a microtask, so the read has no window. The type says `number[]`.
  if (context === "subclass-late-foreach") {
    const [late, h, held] = [kit.name("L"), kit.name("f"), kit.name("a")];
    const fresh = `class ${late} extends Array<number> { override forEach(cb: (v: number, i: number, a: number[]) => void) { queueMicrotask(() => { super.forEach(cb); }); } }`;
    return { hoisted: [expr(domExpr(fresh))], body: [decl(h, fn(stmts)), decl(held, domExpr(`new ${late}(1) as number[]`)), expr(domExpr(`${held}.forEach(${h})`))], view: [] };
  }
  if (context === "arm-tag") {
    const k = kit.name("K");
    return { body: [decl(k, fn([...stmts, ret(domExpr("<i />"))]))], view: [mounted(kit.name("v"), domExpr(`{isServer ? <${k} /> : null}`, ["isServer"]))] };
  }
  if (context === "then-named") {
    const [f, m] = [kit.name("f"), kit.name("m")];
    const flight = domExpr(`new Promise<number>((done) => setTimeout(() => done(1), 1)).then(${f})`);
    return { body: [decl(f, fn([...stmts, ret(num(1))])), decl(m, prim("memo", fn([ret(flight)]))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
  }
  const held = holderForms("K")[context];
  if (held) {
    const k = kit.name("K");
    const form = holderForms(k)[context];
    const on = kit.name("on");
    const value = fn([...stmts, ret(num(1))]);
    const holder = decl(k, form.read === "body" ? value : fn([ret(jsxExpr(child(kit.name("r"), iife(value))))]));
    const web = ["Dynamic", "Portal", "dynamic"].filter((name) => (form.mount ?? form.later(on)).includes(name) || form.prelude?.includes(name));
    const solid = ["Show", "Loading"].filter((name) => (form.mount ?? form.later(on)).includes(`<${name}`));
    return {
      body: [
        holder,
        ...(form.later ? [decl(on, domExpr("createSignal(false)", undefined, ["createSignal"]))] : []),
      ],
      view: [
        ...(form.later ? [button(kit.name("b"), [expr(domExpr(`${on}[1](true)`))])] : []),
        mounted(kit.name("v"), domExpr(form.prelude ? `{(() => { ${form.prelude} return ${form.mount}; })()}` : form.mount ?? form.later(on), web, solid)),
      ],
    };
  }
  if (context.startsWith("on-")) {
    const t = kit.name("t");
    const form = loadingOnForms(t)[context];
    if (!form) return undefined;
    const first = form.first ? [decl(kit.name("w"), domExpr(`(() => { ${form.first} return 1; })()`))] : [];
    return {
      body: [decl(t, domExpr("createSignal(0)[0]", undefined, ["createSignal"])), ...form.body, ...first, decl(kit.name("l"), domExpr(form.tag, undefined, ["Loading"]))],
      view: [child(kit.name("v"), iife(fn([...stmts, ret(num(1))])))],
    };
  }
  return undefined;
}
