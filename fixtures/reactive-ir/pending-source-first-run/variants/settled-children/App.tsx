import { createEffect, createMemo, createSignal } from "solid-js";

// Violation (SC5001): the tree's expressions cannot go pending, so nothing
// holds the first render's tail flush (ADR 0510): a literal child, string
// attributes, and reads of a synchronous memo and a signal of literals the
// body created. The async-memo child of `effect-apply-first-run`'s
// `expression-child` variant stays uncertifiable.
export function Settled() {
  const value = createMemo(async () => 1);
  const label = createMemo(() => "a");
  const [count] = createSignal(2);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return (
    <div title="static">
      <p>{1}</p>
      <p>{label()}</p>
      <p>{count()}</p>
    </div>
  );
}
