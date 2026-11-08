import { Show, createSignal } from "solid-js";
interface Clickable {
  click(): void;
}

export function App() {
  const [count, setCount] = createSignal(0);
  const [on, setOn] = createSignal(false);
  let button!: HTMLButtonElement;
  function Later() {
    const plain: { click(): void } = button;
    const named: Clickable = button;
    plain.click();
    named.click();
    (button as any).click();
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
