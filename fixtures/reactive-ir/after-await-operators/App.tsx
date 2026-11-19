import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ADR 1113 (narrowed by the coordinator's ruling on 889a755fa): an operator written after an async memo's await that runs project code with no call is a
// site: an operand it coerces that Type Facts do not prove a primitive, an `in` whose operands are not both primitives, any `instanceof`, any `delete`.
// `===`, `!==`, `typeof` and `!` run nothing, and a coerced primitive runs nothing. Each definition is written on one line.
const coerced = { toString() { return String(source()); } };
const probed: object = new Proxy({}, { has: () => source() > 0 });
const removable: { k?: number } = new Proxy<{ k?: number }>({}, { deleteProperty: () => source() > 0 });
const mixed: string | { toString(): string } = Math.random() > 2 ? coerced : "x";
const label = "x";
const text: string = String(Math.random());

createRoot(() => {
  // Rows: the coerced object, the probed proxy, the caught value's `instanceof`, the deleted member, an operand that may be an object.
  createMemo(async () => { await pause(); `${coerced}`; return 1; });
  createMemo(async () => { await pause(); "k" in probed; return 1; });
  createMemo(async () => { try { await pause(); } catch (err) { return err instanceof Error ? 1 : 0; } return 1; });
  createMemo(async () => { await pause(); delete removable.k; return 1; });
  createMemo(async () => { await pause(); return `${mixed}`.length; });
  // No rows: strict equality, `typeof`, `!`, a counter compared and incremented, a coerced string.
  createMemo(async () => { await pause(); return label === "y" ? 0 : 1; });
  createMemo(async () => { await pause(); return typeof coerced === "object" && !probed ? 0 : 1; });
  createMemo(async () => { let total = 0; for (let i = 0; i < 2; i++) { await pause(); total += i; } return total; });
  createMemo(async () => { await pause(); return `${text}!`.length; });
});
