// every-deck-of-cards ExplorerPage.tsx:68 (explorer/input.ts:14): the handler is a member of the object a project
// factory returns (`input.handleKeyDown`), a function the factory declares: not a prop, not a literal at the use.
import { createSignal } from "solid-js";

function createExplorerInput(advance: (delta: number) => void) {
  const [touching, setTouching] = createSignal(false);
  function handleKeyDown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown") advance(40);
  }
  return {
    touching,
    handleKeyDown,
    resetTouchDrag(): void {
      setTouching(false);
    },
  };
}

export function Explorer() {
  const [offset, setOffset] = createSignal(0);
  const input = createExplorerInput((delta) => setOffset(offset() + delta));
  return (
    <section tabindex="0" onKeyDown={input.handleKeyDown}>
      <p>{offset()}</p>
    </section>
  );
}
