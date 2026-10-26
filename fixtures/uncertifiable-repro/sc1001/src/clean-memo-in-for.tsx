// Clean. Mirrors donegeon-client web/apps/client/src/components/board/BoardStage.tsx:483-490: a For
// callback creates memos from the item and an outer signal and reads them in JSX.
import { createMemo, createSignal, For } from "solid-js";

export function Stage() {
  const [stacks] = createSignal([{ id: "a", cards: 2 }, { id: "b", cards: 1 }]);
  const [target, setTarget] = createSignal("");
  return (
    <For each={stacks()}>
      {(stack) => {
        const cards = createMemo(() => stack.cards * 2);
        const isTarget = createMemo(() => target() === stack.id);
        return (
          <button type="button" data-target={isTarget()} onClick={() => setTarget(stack.id)}>
            {stack.id}:{cards()}
          </button>
        );
      }}
    </For>
  );
}
