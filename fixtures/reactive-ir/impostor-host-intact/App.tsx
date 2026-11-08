import { createMemo, createSignal } from "solid-js";

const out: unknown[] = [];
out[Number("0")] = () => 0;
const table: Record<string, unknown> = {};
table[String(1)] = () => 0;
export const listener = { addEventListener(_t: string, cb: () => void) { cb(); } };

export function Intact() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    document.addEventListener("click", () => { set(1); });
    window.addEventListener("click", () => { set(2); });
    document.body.addEventListener("click", () => { set(3); });
    return s();
  });
  return <p>{m()}</p>;
}
export function Shadowed(document: { addEventListener(t: string, cb: () => void): void }) {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    document.addEventListener("click", () => { set(1); });
    return s();
  });
  return <p>{m()}</p>;
}

// No route to the receiver: a literal callback stops short of its slot, and
// `reverse`'s answer is discarded. No site here writes `forEach` by name.
const kept = [1, 2];
kept.map((v) => v + 1);
kept.reverse();

export function CleanArity() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { kept.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}

export function ReplaceStandard() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    "x".replace("x", () => { set(2); return ""; });
    "x".replace(/x/, () => { set(3); return ""; });
    return s();
  });
  return <p>{m()}</p>;
}
