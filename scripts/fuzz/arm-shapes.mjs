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
//
// Recall contexts only, base shapes (misuse.mjs BASE_ONLY_CONTEXTS): the question is the host's run, not the spelling at the site.
import { awaitStmt, button, ctl, expr, flowEl, fn, iife, domExpr } from "./model.mjs";

/** `{ [context]: "misuse" | "legal" }`, the side a context is on for the codes it is placed for (`ARM_CODES`). */
export const ARM_CONTEXTS = {
  "barm-gen-spread": "misuse", "barm-gen-handler": "legal", "barm-gen-never": "legal",
  "barm-gen-drain": "misuse", "barm-gen-await": "legal",
  "barm-climb-sync": "misuse", "barm-climb-await": "legal",
};
export const ARM_CONTEXT_NAMES = Object.keys(ARM_CONTEXTS);

/** The codes each context is placed for: the generator contexts on the three window codes, the loop pair on the two that a write decides, the climb on the read. */
export const ARM_CODES = {
  STRICT_READ_UNTRACKED: ["barm-gen-spread", "barm-gen-handler", "barm-gen-never", "barm-climb-sync", "barm-climb-await"],
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["barm-gen-spread", "barm-gen-handler", "barm-gen-never", "barm-gen-drain", "barm-gen-await"],
  ACTION_CALLED_IN_OWNED_SCOPE: ["barm-gen-spread", "barm-gen-handler", "barm-gen-never", "barm-gen-drain", "barm-gen-await"],
};

const gen = (name, stmts) => ({ ...ctl("gstep", stmts), name });

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
    case "barm-climb-await": return { body: [], view: [flowEl(kit.name("v"), "for", [expr(iife(fn([awaitStmt(), expr(iife(fn(stmts)))], { async: true })))])] };
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
