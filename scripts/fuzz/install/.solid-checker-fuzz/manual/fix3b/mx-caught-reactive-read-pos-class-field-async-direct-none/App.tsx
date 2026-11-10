import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  class K9 {
    f = try { a1(); } catch { };
  }
  new K9();
  return (
    <div>
    </div>
  );
}
