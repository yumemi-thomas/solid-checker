import { createSignal, onCleanup } from "solid-js";

// `focus()` fires focus, focusin, blur, focusout (the previously focused
// element, which may be any element, blurs), and `change` on a text control the
// user edited. A handler of one of those types may run on the dispatcher's
// stack, under its owner: its `onCleanup` and its write stay obligations. A
// handler of any other type (click, input, scroll) is never run by it, so its
// `onCleanup` is a proven SC4001 violation and its write is legal.
function focusIt(target: HTMLElement) {
  target.focus();
}

export function Reached() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onBlur={() => { onCleanup(() => undefined); }} />
      <input onFocusIn={() => { onCleanup(() => undefined); }} />
      <input onFocusOut={() => { onCleanup(() => undefined); }} />
      <input onChange={() => { onCleanup(() => undefined); }} />
      <input onFocus={() => { setCount(count() + 1); }} />
      <button onClick={(event) => focusIt(event.currentTarget as HTMLElement)}>focus</button>
    </div>
  );
}

export function Unreached() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <input onInput={() => { onCleanup(() => undefined); }} />
      <input onInput={() => { setCount(count() + 1); }} />
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
