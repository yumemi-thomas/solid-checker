// A classic script (no import or export): its top-level `let` is every
// script's, so another script may write `n = "then"` (review D35 A4).
let n = 0;
(globalThis as any).box[n] = () => {};
