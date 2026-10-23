import { createSignal, onCleanup } from "solid-js";

// ADR 0361 (amended): a callback whose event runs end at more than one
// attribute takes the union of their types. This one is forwarded to an
// `onScroll` and to an `onFocus`; `focus()` fires `focus`, so it may run on the
// dispatcher's stack, under its owner, and keeps its obligations.
function focusIt(target: HTMLElement) {
  target.focus();
}

function Both(props: { on: () => void }) {
  return (
    <div onScroll={props.on}>
      <input onFocus={props.on} />
    </div>
  );
}

export function App() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <Both on={() => { onCleanup(() => undefined); }} />
      <Both on={() => { setCount(count() + 1); }} />
      <button onClick={(event) => focusIt(event.currentTarget as HTMLElement)}>focus</button>
    </div>
  );
}
