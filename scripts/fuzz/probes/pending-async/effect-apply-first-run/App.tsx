// ADR 0505 (from the misuse matrix): a user effect's first apply, in the tail flush of a module-scope mount that renders a static tree, reads an async memo created earlier in the body while it is pending (PENDING_ASYNC_UNTRACKED_READ).
import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const pair = [0, a1] as const;
  const [, useIt] = pair;
  createEffect(() => {
    return 1;
  }, (v) => {
    useIt();
  });
  return (
    <div>
    </div>
  );
}
