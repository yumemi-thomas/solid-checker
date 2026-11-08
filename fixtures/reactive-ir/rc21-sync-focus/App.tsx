import { Show, createSignal } from "solid-js";


export function Page() {
  const [count, setCount] = createSignal(0);
  const [on, setOn] = createSignal(false);
  let input!: HTMLInputElement;
  function Later() {
    input.focus();
    return <i>x</i>;
  }
  return (
    <div>
      <input ref={input} onFocus={() => setCount(count() + 1)} />
      <button onClick={() => setOn(true)}>go</button>
      <Show when={on()}>
        <Later />
      </Show>
    </div>
  );
}
