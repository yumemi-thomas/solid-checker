// Clean. Mirrors compass-ui apps/ui/src/components/CoachTip.tsx:58: a keyed Show hands its child
// the value, and the reads sit in JSX; the accessor read in a non-keyed child's body is SC1001 (read-in-show-child).
import { createSignal, Show } from "solid-js";

export function Tip() {
  const [chord, setChord] = createSignal<string | undefined>("ctrl k");
  return (
    <Show when={chord()} keyed>
      {(resolved) => <button onClick={() => setChord(undefined)}>{resolved}</button>}
    </Show>
  );
}
