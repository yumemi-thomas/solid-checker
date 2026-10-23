import { createSignal, onCleanup } from "solid-js";

// `Reflect.apply` of an owner-specific member: the same value-run dispatch.
function press(dialog: HTMLDialogElement) {
  Reflect.apply(dialog.showModal, dialog, []);
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
