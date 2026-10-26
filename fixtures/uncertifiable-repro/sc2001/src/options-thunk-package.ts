// inferay FileSearch/index.tsx:78-87,99: a handler stored in an options object that a createMemo thunk returns.
import { createMemo, createSignal } from "solid-js";

export function createSearch(results: () => string[]) {
  const [selected, setSelected] = createSignal(0);
  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "ArrowDown") setSelected((index) => Math.min(results().length - 1, index + 1));
  };
  const inputProps = createMemo(() => ({ onKeyDown: handleKeyDown }));
  return { selected, inputProps };
}
