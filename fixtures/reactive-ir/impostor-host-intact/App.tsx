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
