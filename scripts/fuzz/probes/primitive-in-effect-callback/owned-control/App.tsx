// ADR 0483 control: the same memo under runWithOwner(owner) is owned; nothing throws. `#w` shows "ok".
import { createEffect, createMemo, createSignal, getOwner, runWithOwner } from "solid-js";

export default function App() {
  const [count] = createSignal(0);
  const [seen, setSeen] = createSignal("wait");
  const owner = getOwner();
  createEffect(
    () => count(),
    (value) => {
      runWithOwner(owner, () => createMemo(() => value * 2));
      setSeen("ok");
    },
  );
  return <span id="w">{seen()}</span>;
}
