import { createSignal, onCleanup } from "solid-js";

// Contrast: owner-specific spellings that are plain reads or project members
// are no dispatch (`dialog.open` is a property; `store.close.call` is the
// project's own function), so every handler keeps its proof.
const store = { close() {} };
function press(dialog: HTMLDialogElement) {
  store.close.call(dialog);
  return dialog.open;
}

export function Reached() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input type="checkbox" onBeforeToggle={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onBeforeToggle={() => { setCount(count() + 1); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <div onScroll={() => { setCount(count() + 1); }}>scroll write</div>
      <button onClick={(event) => press(event.currentTarget as unknown as HTMLDialogElement)}>press</button>
    </div>
  );
}
