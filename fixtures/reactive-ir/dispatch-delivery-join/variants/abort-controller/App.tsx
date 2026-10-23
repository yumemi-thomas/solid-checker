import { createSignal, onCleanup } from "solid-js";

// `AbortController.abort`, `XMLHttpRequest.open`/`abort`/`send` and
// `FileReader.abort` fire their events at the signal, the request and the
// reader, which are no elements: no compiler handler (an element attribute)
// listens there. Every element handler keeps its proof, whatever its type.
export function Abort() {
  const [count, setCount] = createSignal(0);
  const controller = new AbortController();
  controller.abort();
  const request = new XMLHttpRequest();
  request.open("GET", "/data");
  request.send();
  request.abort();
  const reader = new FileReader();
  reader.abort();
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onChange={() => { onCleanup(() => undefined); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <input onFocus={() => { setCount(count() + 1); }} />
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
