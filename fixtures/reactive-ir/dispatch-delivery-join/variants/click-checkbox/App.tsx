import { createSignal, onCleanup } from "solid-js";

// `click()` runs the element's activation behavior: the click itself, `input`
// and `change` on a checkbox or radio, and the focus move a label's control
// takes. Their handlers stay obligations; a scroll handler is never reached.
function press(target: HTMLInputElement) {
  target.click();
}

export function Checkbox() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input type="checkbox" onClick={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onInput={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onChange={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onChange={() => { setCount(count() + 1); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <div onScroll={() => { setCount(count() + 1); }}>scroll write</div>
      <button onClick={(event) => press(event.currentTarget as unknown as HTMLInputElement)}>press</button>
    </div>
  );
}
