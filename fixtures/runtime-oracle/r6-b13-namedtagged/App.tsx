import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
import { createMemo, untrack, createEffect } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
export function C() {
  function press(_s: TemplateStringsArray) {
    helper(opaque);
    return "";
  }
  const t = press`x`;
  return <button onClick={press as unknown as () => void} title={t} />;
}
