// ADR 0508 (from the mount axis): a user effect's first apply, in a child an inert parent renders in a one-row <For>, reads an async memo created earlier in the body while it is pending (PENDING_ASYNC_UNTRACKED_READ).
import { For, createEffect, createMemo } from "solid-js";

function C1() {
  const a1 = createMemo(async () => 1);
  createEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return (
    <div>
    </div>
  );
}

export function P1() {
  return (
    <div>
      <For each={[1]}>
        {() => <C1 />}
      </For>
    </div>
  );
}
