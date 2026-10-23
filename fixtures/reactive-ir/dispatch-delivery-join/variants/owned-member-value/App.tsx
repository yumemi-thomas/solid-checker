import { createSignal, onCleanup } from "solid-js";

// An owner-specific member invoked as a value (`.call`) runs wherever it is
// called: `showModal` fires `beforetoggle`, so the `onBeforeToggle` handlers
// stay obligations, as does every other handler (the dispatch is no site's).
function press(dialog: HTMLDialogElement) {
  dialog.showModal.call(dialog);
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
