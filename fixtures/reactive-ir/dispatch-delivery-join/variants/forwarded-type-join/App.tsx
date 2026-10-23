import { createSignal, onCleanup } from "solid-js";

// ADR 0361 (amended): a callback forwarded through a component prop ends, by
// ADR 0319's walk, at an intrinsic `on…` attribute; its type is that
// attribute's. `focus()` fires focus, focusin, blur, focusout, change and
// compositionend: the callback forwarded to `onScroll` is never run by it, so
// its `onCleanup` is a proven SC4001 violation and its write is legal; the
// one forwarded to `onFocus` may run on the dispatcher's stack, under its
// owner, and keeps both obligations.
function focusIt(target: HTMLElement) {
  target.focus();
}

function Scroller(props: { onScrolled: () => void }) {
  return <div onScroll={props.onScrolled}>scroll</div>;
}

function Focuser(props: { onFocused: () => void }) {
  return <input onFocus={props.onFocused} />;
}

export function App() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <Scroller onScrolled={() => { onCleanup(() => undefined); }} />
      <Scroller onScrolled={() => { setCount(count() + 1); }} />
      <Focuser onFocused={() => { onCleanup(() => undefined); }} />
      <Focuser onFocused={() => { setCount(count() + 1); }} />
      <button onClick={(event) => focusIt(event.currentTarget as HTMLElement)}>focus</button>
    </div>
  );
}
