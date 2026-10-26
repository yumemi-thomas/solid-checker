// Clean. Mirrors ai-memory-ui src/components/tooltip.tsx:62-75: a Show with a function child renders a
// Portal holding the panel; the open signal is set by pointer handlers (a click toggle here).
import { createSignal, Show } from "solid-js";
import { Portal } from "@solidjs/web";

export function Tip() {
  const [open, setOpen] = createSignal(false);
  const [content] = createSignal("tip text");
  return (
    <button type="button" data-probe="trigger" onClick={() => setOpen(!open())}>
      hover
      <Show when={open() && content()}>
        {(text) => (
          <Portal>
            <div role="tooltip">{text()}</div>
          </Portal>
        )}
      </Show>
    </button>
  );
}
