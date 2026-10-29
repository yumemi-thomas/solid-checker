// ADR 0510 (wrong rule, the combination grid): an action created between the memo and the effect kept ADR 0504's proof off the
// wrapper's read, and its strict read stayed SC1001 while rc.17 throws PENDING_ASYNC_UNTRACKED_READ first; it is an obligation now.
import { action, createMemo, createRenderEffect, createSignal, refresh } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const [w1, setW1] = createSignal(0);
  const a2 = action(function* () { setW1(7); });
  const m1 = createMemo(() => 1);
  const wrapUse = () => { a1(); };
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    wrapUse();
  });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        a2();
      }}>go</button>
      <button data-act="b2" onClick={() => {
        refresh(m1);
      }}>go</button>
    </div>
  );
}
