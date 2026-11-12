// The later-assignment shapes (batch 11): the site is the body of a function the program ASSIGNS to a variable, a member or an element after the place that declares it, and the
// host's callback then calls it through that name. A `let` holds the function written last, not its initializer, and the calls of the name resolve to the initializer (or to nothing):
// no call edge placed the assigned function, and `runs_unplaced_once` (static_api.rs) read a function with no binding name as "nobody calls it". Chrome (rc.21, the real oracle)
// confirmed six false cleans on main 1a87feae8: `let h = () => {}; h = () => flush(); h();` in an effect's apply and in an action's step, certified with no row, while Chrome
// reports FLUSH_IN_EFFECT_CALLBACK and FLUSH_IN_ACTION. The member forms were rowed already (ADR 0535); a variable's was not.
//
//   la-<form>-<host>          misuse   the assigned function runs where the host's callback calls it
//   la-<form>-<host>-never    legal    the same text with the function never called (or replaced before the call): no site runs
//     forms   let-later     `let h = () => {}; h = () => { <site> }; h();`
//             let-noinit    `let h: () => void; h = () => { <site> }; h();`
//             let-other     `let h = () => {}; const set = () => { h = () => { <site> }; }; set(); h();`  (assigned in another function)
//             let-early     `let h = () => { <site> }; h(); h = () => {};`  (called before the reassignment: the twin reassigns first)
//             member-later  `const o = { f: () => {} }; o.f = () => { <site> }; o.f();`   (a control: rowed since ADR 0535)
//             elem-later    `const a = [() => {}]; a[0] = () => { <site> }; a[0]();`      (a control)
//   hosts   effect-apply (an effect's apply: SC2008, SC6002), action (an action's step, run from a button: SC2006), tracked and settled (SC3001's leaf owners)
//
// What Chrome reports in each cell is what the judge compares the checker with (a `-misuse` cell Chrome keeps quiet on is no false clean). Recall contexts, the direct binding bare
// (`laShape`), not crossed by the census, out of api-fire's plan.
import { button, call, contain, decl, expr, fn, num, prim, ref, ret } from "./model.mjs";

export const LA_FORMS = ["let-later", "let-noinit", "let-other", "let-early", "member-later", "elem-later"];
export const LA_HOSTS = ["effect-apply", "action", "tracked", "settled"];
/** The hosts each code is judged in: the codes the host's callback reports for a site in it. */
export const LA_CODES = {
  FLUSH_IN_EFFECT_CALLBACK: ["effect-apply"],
  PRIMITIVE_IN_EFFECT_CALLBACK: ["effect-apply"],
  FLUSH_IN_ACTION: ["action"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["tracked", "settled"],
  CLEANUP_IN_FORBIDDEN_SCOPE: ["tracked", "settled"],
  PRIMITIVE_IN_FORBIDDEN_SCOPE: ["tracked", "settled"],
};
/** `{ [context]: "misuse" | "legal" }`. */
export const LA_CONTEXTS = Object.fromEntries(LA_FORMS.flatMap((form) => LA_HOSTS.flatMap((host) => [[`la-${form}-${host}`, "misuse"], [`la-${form}-${host}-never`, "legal"]])));
export const LA_CONTEXT_NAMES = Object.keys(LA_CONTEXTS);

const NAME = new RegExp(`^la-(${[...LA_FORMS].sort((a, b) => b.length - a.length).join("|")})-(${LA_HOSTS.join("|")})(-never)?$`);
/** `{ form, host, never }` of an `la-*` context name, or null. */
export const laParts = (context) => {
  const match = NAME.exec(context);
  return match ? { form: match[1], host: match[2], never: Boolean(match[3]) } : null;
};

/** The base shapes an `la-*` cell takes: the direct binding, no layer. */
export function laShape(context, shape) {
  return !laParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/**
 * The frame of a form (model.mjs FRAME_CONTAINER_KINDS): `head` precedes the assigned function's body, `close` follows it, `run` calls the function, `idle` leaves it uncalled
 * (a twin: its names stay used, so the program is the misuse text with no site run).
 */
const FRAMES = {
  "let-later": (n) => { const h = n("h"); return { head: `let ${h} = () => {};\n${h} = () => `, close: ";", run: [`${h}();`], idle: [`void ${h};`] }; },
  "let-noinit": (n) => { const h = n("h"); return { head: `let ${h}: () => void;\n${h} = () => `, close: ";", run: [`${h}();`], idle: [`void ${h};`] }; },
  "let-other": (n) => { const h = n("h"); const s = n("s"); return { head: `let ${h} = () => {};\nconst ${s} = () => {\n  ${h} = () => `, close: ";\n};", nest: 1, run: [`${s}();`, `${h}();`], idle: [`void ${s};`, `void ${h};`] }; },
  "let-early": (n) => { const h = n("h"); return { head: `let ${h} = () => `, close: ";", run: [`${h}();`, `${h} = () => {};`], idle: [`${h} = () => {};`, `${h}();`] }; },
  "member-later": (n) => { const o = n("o"); return { head: `const ${o} = { f: () => {} };\n${o}.f = () => `, close: ";", run: [`${o}.f();`], idle: [`void ${o};`] }; },
  "elem-later": (n) => { const a = n("a"); return { head: `const ${a} = [() => {}];\n${a}[0] = () => `, close: ";", run: [`${a}[0]();`], idle: [`void ${a};`] }; },
};

/** The host's own placement of `held` (the plain primitive-layer hosts, primitive-layer-shapes.mjs `plContext`). */
function placeIn(host, held, kit) {
  switch (host) {
    case "tracked": return { body: [expr(prim("tracked", fn(held)))], view: [] };
    case "settled": return { body: [expr(prim("settled", fn(held)))], view: [] };
    case "effect-apply": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn(held) }))], view: [] };
    case "action": {
      const a = kit.name("a");
      return { body: [decl(a, prim("action", fn(held)))], view: [button(kit.name("b"), [expr(call(ref(a)))])] };
    }
    default: throw new Error(`no later-assignment host ${host}`);
  }
}

/** The placement of `stmts` for an `la-*` context (`{ body, view }`), or undefined for any other. */
export function laContext(context, stmts, kit) {
  const parts = laParts(context);
  if (!parts) return undefined;
  const { run, idle, ...frame } = FRAMES[parts.form](kit.name);
  return placeIn(parts.host, [contain("frame", 9, stmts, { ...frame, then: parts.never ? idle : run })], kit);
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function laPlacement() {
  return Object.fromEntries(Object.entries(LA_CODES).map(([code, hosts]) => {
    const contexts = LA_CONTEXT_NAMES.filter((name) => hosts.includes(laParts(name).host));
    return [code, { misuse: contexts.filter((name) => LA_CONTEXTS[name] === "misuse"), legal: contexts.filter((name) => LA_CONTEXTS[name] === "legal") }];
  }));
}
