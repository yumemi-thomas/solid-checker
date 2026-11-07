import { createMemo, createSignal } from "solid-js";

const own = EventTarget.prototype.addEventListener;
const sync = function (this: any, t: string, cb: any, ...r: any[]) {
  if (t === "x-sync") cb();
  else own.call(this, t, cb, ...r);
};
(document as any).addEventListener = sync;
const alias: any = document.body;
alias.addEventListener = sync;
const helper = (o: any) => { o.addEventListener = sync; };
helper(window);

export function HostWritten() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    document.addEventListener("x-sync", () => { set(1); });
    window.addEventListener("x-sync", () => { set(2); });
    document.body.addEventListener("x-sync", () => { set(3); });
    return s();
  });
  return <p>{m()}</p>;
}
