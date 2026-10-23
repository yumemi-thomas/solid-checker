import { createSignal, onCleanup } from "solid-js";

// One function is an element's handler and a `window` listener. The window
// dispatch below runs the listener on its caller's stack, inside the owner of
// the component body, so a write in it may run owned: it stays conditional. The
// element's own click runs it with no owner whatever else reaches it, so the
// `onCleanup` in `shared` keeps its proof (as in the root's `SharedHandler`).
const [count, setCount] = createSignal(0);

function shared() {
  onCleanup(() => undefined);
}

function sharedWrite() {
  setCount(1);
}

export function SharedHandler() {
  window.addEventListener("app:ping", shared);
  window.addEventListener("app:ping", sharedWrite);
  window.dispatchEvent(new CustomEvent("app:ping"));
  return (
    <div>
      <button onClick={shared}>cleanup</button>
      <button onClick={sharedWrite}>write {count()}</button>
    </div>
  );
}

// A `window` listener no element names: the dispatch runs it on the stack of
// the component body, so it may be owned. The program's dispatch being
// supported for elements changes nothing here.
export function ListenerOnly() {
  window.addEventListener("app:pong", () => {
    onCleanup(() => undefined);
    setCount(2);
  });
  window.dispatchEvent(new CustomEvent("app:pong"));
  return <p>listener</p>;
}
