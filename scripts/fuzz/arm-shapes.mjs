// The arm-coverage shapes (ADR 0935): contexts for tagged classifier arms that no fixture program and no other pair of the registry reached
// (the arm-ablation of the fixture projects listed them as unwitnessed: scripts/proof-ablation.mjs). Each context has a twin that differs only in the
// fact the arm's premise claims, so the arm answers silent in a legal program and the twin is not silent.
//
//   generators (ADR 0851)   the site past the first `yield` of a named generator. Calling a generator runs none of its body; each step runs the next
//                           segment on the stack of the code that makes it. `callee_roles.generator-steps` takes the role at the steps;
//                           `generator-never-stepped` is the generator nothing steps; `write_role_walk.unreported-generator` a write past a
//                           `yield` an async caller steps across one of its suspensions.
//     barm-gen-spread       misuse  `void [...g()]` in the body: every step runs in the body's window
//     barm-gen-handler      legal   the same spread inside a click handler: the steps run with no owner and no window
//     barm-gen-never        legal   the generator is declared and nothing steps it
//     barm-gen-drain        misuse  a sync generator drained by `for…of` in an async function the body calls: the steps run on the body's stack
//     barm-gen-await        legal   the same loop awaiting between steps: each later step runs on the continuation (writes and actions only: a read
//                                   in that shape is an obligation, not a clean)
//   flow climb after an await (callee_roles.rs `control-flow-climb-after-await`)   a read in a layer that runs in place inside a layer of a
//                           `<For>` row's body
//     barm-climb-sync       misuse  both layers run in the row body's window
//     barm-climb-await      legal   the outer layer awaits first, so the inner one runs on a later task
//   a `forEach`-carried entry (ADR 1124 round 2, write_roles.rs `role_entries_complete.foreach-carried`)   a function named at an effect's compute
//                           (which admits no role arm, so the census is asked by the tracked-position silence itself) and carried by the
//                           standard `forEach` of an array literal: the census counts the carrier's call as an entry, so the tracked position is
//                           every way in and the read is silent there; the carried call places it where the `forEach` runs
//     barm-carried-body     misuse  `const f = () => { <site> }; createEffect(f, () => {}); [f].forEach((g) => g());` in the body: the carried call runs it untracked
//     barm-carried-handler  legal   the same `forEach` inside a click handler: the carried call runs it with no observer, where a read is legal
//   a standard member replaced by code built from a string (ADR 1137; the review of ADR 1124, round 2, N1)   `Array.prototype.forEach` replaced through
//                           `(0, eval)("…")` or `new Function("…")()` by one that defers its callbacks while a flag is set; the site is in an inline
//                           `[1].forEach(() => { <site> })` inside `untrack`, and the body then runs what was deferred: the read runs in the body's window
//     barm-strpatch-eval    misuse  the patch through indirect `eval`
//     barm-strpatch-newfn   misuse  the patch through `new Function`
//     barm-strpatch-none    legal   no patch: the callback runs inside `untrack`, where a read is legal
//   an action of a context's `[store, { actions }]` literal (ADR 1147, arm 3, `context-provided-store-tuple`)   the site is the body of `const act = () => { <site> }`,
//                           provided as `<Cx value={[st, { act }] as const}>` around a local reader `R` that takes `const [, { act: a }] = useContext(Cx)`: the
//                           arm places the action's runs at the reader's calls of `a`
//     barm-ctxstore-memo    misuse  the reader calls `a()` in a `createMemo`: the write runs in the memo's compute
//     barm-ctxstore-handler legal   the reader calls `a()` in a click handler
//     barm-ctxstore-alias   misuse  the memo twin through an alias (`const f = a`): the arm refuses it (an action local used as a value), so it stays uncertifiable
//
// Recall contexts only, base shapes (misuse.mjs BASE_ONLY_CONTEXTS): the question is the host's run, not the spelling at the site.
import { awaitStmt, button, contain, ctl, expr, flowEl, fn, iife, domExpr, mounted } from "./model.mjs";

/** `{ [context]: "misuse" | "legal" }`, the side a context is on for the codes it is placed for (`ARM_CODES`). */
export const ARM_CONTEXTS = {
  "barm-gen-spread": "misuse", "barm-gen-handler": "legal", "barm-gen-never": "legal",
  "barm-gen-drain": "misuse", "barm-gen-await": "legal",
  "barm-climb-sync": "misuse", "barm-climb-await": "legal",
  "barm-carried-body": "misuse", "barm-carried-handler": "legal",
  "barm-strpatch-eval": "misuse", "barm-strpatch-newfn": "misuse", "barm-strpatch-none": "legal",
  "barm-ctxstore-memo": "misuse", "barm-ctxstore-handler": "legal", "barm-ctxstore-alias": "misuse",
};
export const ARM_CONTEXT_NAMES = Object.keys(ARM_CONTEXTS);

/** The codes each context is placed for: the generator contexts on the three window codes, the loop pair on the two that a write decides, the climb on the read. */
export const ARM_CODES = {
  STRICT_READ_UNTRACKED: ["barm-gen-spread", "barm-gen-handler", "barm-gen-never", "barm-climb-sync", "barm-climb-await", "barm-carried-body", "barm-carried-handler",
    "barm-strpatch-eval", "barm-strpatch-newfn", "barm-strpatch-none"],
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["barm-gen-spread", "barm-gen-handler", "barm-gen-never", "barm-gen-drain", "barm-gen-await",
    "barm-ctxstore-memo", "barm-ctxstore-handler", "barm-ctxstore-alias"],
  ACTION_CALLED_IN_OWNED_SCOPE: ["barm-gen-spread", "barm-gen-handler", "barm-gen-never", "barm-gen-drain", "barm-gen-await"],
};

/**
 * ADR 1124 round 2: the read variants a context takes beside its base shapes (matrix.mjs `recallAxes`), per code. The carried pair's
 * question is the entry census, which a store's member read asks outside the classifier's own decision (the member silence), so the
 * pair takes `store-link` (a member read of a store) beside the base signal and memo reads.
 */
export const ARM_VARIANTS = {
  STRICT_READ_UNTRACKED: { "barm-carried-body": ["store-link"], "barm-carried-handler": ["store-link"] },
};

const gen = (name, stmts) => ({ ...ctl("gstep", stmts), name });
/** `const <f> = () => { <stmts> }; createEffect(<f>, () => {});` (the carried pair's function, named at an effect's compute), then `after`. */
const carried = (f, stmts, after) => contain("frame", 0, stmts, { head: `const ${f} = () => `, close: ";", then: [`createEffect(${f}, () => {});`, ...after], imports: ["createEffect"] });

/**
 * ADR 1137: the string-patch contexts. `install` is the statement that replaces `forEach` (none for the legal twin); the site runs in an inline `forEach` callback inside
 * `untrack`, and what the patched `forEach` deferred runs after it, in the body.
 */
const strPatch = (kit, stmts, install) => {
  const flag = kit.name("sf");
  const late = kit.name("sl");
  const orig = kit.name("so");
  const hook = `__${kit.name("sp")}`;
  const setup = [
    `const ${flag} = { on: false };`,
    `const ${late}: Array<() => void> = [];`,
    `const ${orig} = Array.prototype.forEach;`,
    `(globalThis as any).${hook} = function (this: unknown[], fn: any, t?: unknown) { if (${flag}.on) { ${late}.push(() => ${orig}.call(this, fn, t)); return; } return ${orig}.call(this, fn, t); };`,
    ...(install ? [install(`Array.prototype.forEach = (globalThis as any).${hook}`.replace("(globalThis as any)", "globalThis"))] : []),
  ];
  return contain("frame", 0, stmts, {
    head: `${setup.join("\n")}\nuntrack(() => { ${flag}.on = true; [1].forEach(() => `,
    close: `); ${flag}.on = false; });`,
    then: [`for (const f of ${late}.splice(0)) f();`],
    imports: ["untrack"],
  });
};

/**
 * ADR 1147 (arm 3): the context-store pair. The site is the action's body; the reader `R` destructures the provided literal's slot 1 and calls the action's local
 * in a memo (`use` "memo"), through an alias in a memo ("alias"), or in a click handler ("handler").
 */
const ctxStore = (kit, stmts, use) => {
  const cx = kit.name("Cx");
  const st = kit.name("st");
  const act = kit.name("act");
  const r = `R${kit.name("r")}`;
  const a = kit.name("ax");
  const run = use === "alias" ? `const ${a}f = ${a}; createMemo(() => { ${a}f(); return 0; });` : use === "memo" ? `createMemo(() => { ${a}(); return 0; });` : "";
  const out = use === "handler" ? `<button data-act="${kit.name("ab")}" onClick={() => ${a}()}>go</button>` : "<i />";
  return {
    body: [contain("frame", 0, stmts, {
      head: `const ${cx} = createContext<readonly [{ n: number }, { ${act}: () => void }]>();\nconst [${st}] = createStore({ n: 0 });\nconst ${act} = () => `,
      close: ";",
      then: [`function ${r}() { const [, { ${act}: ${a} }] = useContext(${cx}); ${run} return ${out}; }`],
      imports: ["createContext", "createStore", "useContext", ...(use === "handler" ? [] : ["createMemo"])],
    })],
    view: [mounted(kit.name("v"), domExpr(`<${cx} value={[${st}, { ${act} }] as const}><${r} /></${cx}>`))],
  };
};

/** The placement of `stmts` for a `barm-*` context (`{ body, view }`), or undefined for any other. */
export function armContext(context, stmts, kit) {
  if (!ARM_CONTEXTS[context]) return undefined;
  const g = kit.name("g");
  const b = kit.name("b");
  const h = kit.name("h");
  switch (context) {
    case "barm-gen-spread": return { body: [gen(g, stmts), expr(domExpr(`void [...${g}()]`))], view: [] };
    case "barm-gen-handler": return { body: [gen(g, stmts)], view: [button(b, [expr(domExpr(`void [...${g}()]`))])] };
    case "barm-gen-never": return { body: [gen(g, stmts)], view: [] };
    case "barm-gen-drain":
    case "barm-gen-await": {
      const pause = context === "barm-gen-await" ? "await Promise.resolve(); " : "";
      return { body: [gen(g, stmts), expr(domExpr(`async function ${h}() { for (const y of ${g}()) { ${pause}void y; } }`)), expr(domExpr(`void ${h}()`))], view: [] };
    }
    case "barm-climb-sync": return { body: [], view: [flowEl(kit.name("v"), "for", [expr(iife(fn([expr(iife(fn(stmts)))])))])] };
    case "barm-carried-body": { const f = kit.name("f"); return { body: [carried(f, stmts, [`[${f}].forEach((g) => g());`])], view: [] }; }
    case "barm-carried-handler": { const f = kit.name("f"); return { body: [carried(f, stmts, [])], view: [button(b, [expr(domExpr(`[${f}].forEach((g) => g())`))])] }; }
    case "barm-climb-await": return { body: [], view: [flowEl(kit.name("v"), "for", [expr(iife(fn([awaitStmt(), expr(iife(fn(stmts)))], { async: true })))])] };
    case "barm-strpatch-eval": return { body: [strPatch(kit, stmts, (code) => `(0, eval)(${JSON.stringify(code)});`)], view: [] };
    case "barm-strpatch-newfn": return { body: [strPatch(kit, stmts, (code) => `new Function(${JSON.stringify(code)})();`)], view: [] };
    case "barm-strpatch-none": return { body: [strPatch(kit, stmts, null)], view: [] };
    case "barm-ctxstore-memo": return ctxStore(kit, stmts, "memo");
    case "barm-ctxstore-handler": return ctxStore(kit, stmts, "handler");
    case "barm-ctxstore-alias": return ctxStore(kit, stmts, "alias");
    default: return undefined;
  }
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function armPlacement() {
  return Object.fromEntries(Object.entries(ARM_CODES).map(([code, contexts]) => [code, {
    misuse: contexts.filter((context) => ARM_CONTEXTS[context] === "misuse"),
    legal: contexts.filter((context) => ARM_CONTEXTS[context] === "legal"),
  }]));
}
