// The flip's read track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_READ_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipReadContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
import { child, decl, domExpr, fn, iife, num, ret } from "./model.mjs";

export const FLIP_READ_CONTEXTS = {
  LOADING_ON_ACCESSOR: ["on-any", "on-unknown", "on-spread", "on-json", "on-prop", "on-call"],
};

// A value of the boundary's own, beside the site: the context is the way `on` is handed. The boundary is built in the body: a view child must be a number.
const loadingOnForms = (t) => ({
  "on-any": { body: [decl("f", domExpr(`${t} as any`))], tag: "<Loading on={f} fallback={0}><i /></Loading>" },
  "on-unknown": { body: [decl("f", domExpr(`${t} as unknown`))], tag: "<Loading on={f} fallback={0}><i /></Loading>" },
  "on-spread": { body: [decl("p", domExpr(`{ on: ${t} }`))], tag: "<Loading {...p} fallback={0}><i /></Loading>" },
  "on-json": { body: [decl("o", domExpr(`JSON.parse("{}")`))], tag: "<Loading on={o.f} fallback={0}><i /></Loading>", first: `o.f = ${t};` },
  "on-prop": { body: [decl("B", domExpr("(props: { on: unknown }) => <Loading on={props.on} fallback={0}><i /></Loading>"))], tag: `<B on={${t}} />` },
  "on-call": { body: [], tag: `Loading({ on: ${t}, fallback: 0, children: <i /> }) as any` },
});

export function flipReadContext(context, stmts, kit) {
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
