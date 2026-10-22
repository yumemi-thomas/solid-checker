import { onCleanup } from "solid-js";

function handlers(cb: () => void) {
  return { onScroll: () => cb() };
}

// The spread's holder is a helper's result: the element's handlers are not
// known, so the dispatch cannot be joined to them.
export function UnresolvedSpread() {
  const h = handlers(() => { onCleanup(() => undefined); });
  const el = (<div {...h} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
