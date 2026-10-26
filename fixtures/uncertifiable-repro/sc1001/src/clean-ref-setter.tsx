// Clean. Mirrors donegeon-client web/apps/client/src/components/board/BoardSettingsModal.tsx:225: a signal
// setter is the ref, and a click handler reads the element signal at event time.
import { createSignal } from "solid-js";

export function Board() {
  const [input, setInput] = createSignal<HTMLInputElement>();
  const [name, setName] = createSignal("");
  return (
    <div>
      <input ref={setInput} value={name()} onInput={(event) => setName(event.currentTarget.value)} />
      <button type="button" onClick={() => input()?.focus()}>focus</button>
    </div>
  );
}
