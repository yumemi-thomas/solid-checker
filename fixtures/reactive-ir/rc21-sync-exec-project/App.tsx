import { Show, createSignal } from "solid-js";
const editor = { execCommand(name: string) {
  return name;
} };

export function App() {
  const [count, setCount] = createSignal(0);
  const [on, setOn] = createSignal(false);
  function Later() {
    editor.execCommand("insertText");
    return <i>x</i>;
  }
  return (
    <div>
      <input onInput={() => setCount(count() + 1)} />
      <button onClick={() => setOn(true)}>go</button>
      <Show when={on()}>
        <Later />
      </Show>
    </div>
  );
}
