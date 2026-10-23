import { createSignal, onCleanup } from "solid-js";

// ADR 0366: the helper shape needs every call after a dominating `await`.
// `saveBlob` is also called synchronously from `saveNow`, which may run in an
// owned scope: the `click()` set fires, so the click and change handlers are
// obligations; so is the focus handler (the focus move).
declare function download(): Promise<Blob>;

function saveBlob(blob: Blob): void {
  const anchor = document.createElement("a");
  anchor.href = URL.createObjectURL(blob);
  anchor.click();
}

export async function backup() {
  const blob = await download();
  saveBlob(blob);
}

export function saveNow(blob: Blob) {
  saveBlob(blob);
}

export function Handlers() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onChange={() => { onCleanup(() => undefined); }} />
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
