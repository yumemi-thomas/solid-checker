import { createSignal, onCleanup } from "solid-js";

// ADR 0366: the ai-memory-ui shape. `saveBlob` is a synchronous, module-local
// helper whose every call follows a dominating `await` in its caller, so its
// `anchor.click()` runs on a promise continuation's stack, with no owner. It
// fires nothing: every handler keeps its proof.
declare function download(): Promise<Blob>;

function saveBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

export async function backup() {
  try {
    const blob = await download();
    saveBlob(blob, "backup.json");
  } catch {
    void 0;
  }
}

export async function exportAll() {
  const blob = await download();
  saveBlob(blob, "all.json");
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
