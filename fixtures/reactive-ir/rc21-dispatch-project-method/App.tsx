import { Show, createSignal } from "solid-js";
class Lens {
  click() {
    return 1;
  }
}

export function App() {
  const [count, setCount] = createSignal(0);
  const [on, setOn] = createSignal(false);
  let button!: HTMLButtonElement;
  function Later() {
    new Lens().click();
    ({ click() { return 2; } }).click();
    ({ click: () => 3 }).click();
    return <i>x</i>;
  }
  return (
    <div>
      <button ref={button} onClick={() => setCount(count() + 1)}>a</button>
      <button onPointerDown={() => setOn(true)}>go</button>
      <Show when={on()}>
        <Later />
      </Show>
    </div>
  );
}
