import { createMemo, untrack } from "solid-js";
const flag = { on: false };
const late: Array<() => void> = [];
const orig = Array.prototype.forEach;
const P = function (this: unknown[], fn: (v: unknown, i: number, a: unknown[]) => void, t?: unknown) { if (flag.on) { late.push(() => orig.call(this, fn, t)); return; } return orig.call(this, fn, t); };
(globalThis as any).__p = P;
const tick = () => {}; const later = (done: () => void) => setTimeout(done, 0); setTimeout(tick, 0); later(tick);
export function InlineRead() {
  const base = createMemo(() => 1);
  untrack(() => { flag.on = true; [1].forEach(() => { base(); }); flag.on = false; });
  for (const f of late.splice(0)) f();
  return <div />;
}
