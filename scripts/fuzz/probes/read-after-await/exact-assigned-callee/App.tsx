// ADR 0517: an assignment callee read after an await (UNTRACKED_READ_AFTER_AWAIT).
import { createEffect, createMemo, createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(3);
  let slot: typeof r | undefined;
  const m = createMemo(async () => {
    await Promise.resolve();
    (slot = r)();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <span id="w">{r()}</span>;
}
