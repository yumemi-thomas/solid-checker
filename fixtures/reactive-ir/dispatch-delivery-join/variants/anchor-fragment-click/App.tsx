import { createSignal, onCleanup } from "solid-js";

// A same-document fragment navigation scrolls synchronously, and "scroll to the
// fragment" reveals a `hidden=until-found` ancestor, firing `beforematch`
// (HTML Standard, ancestor revealing algorithm): the `onBeforeMatch` handlers
// stay obligations.
function press(target: HTMLAnchorElement) {
  target.click();
}

export function Reached() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input type="checkbox" onBeforeMatch={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onBeforeMatch={() => { setCount(count() + 1); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <div onScroll={() => { setCount(count() + 1); }}>scroll write</div>
      <button onClick={(event) => press(event.currentTarget as unknown as HTMLAnchorElement)}>press</button>
    </div>
  );
}
