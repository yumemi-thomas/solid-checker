// The flip's write track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_WRITE_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipWriteContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
export const FLIP_WRITE_CONTEXTS = {};
export function flipWriteContext(_context, _stmts, _kit) { return undefined; }
