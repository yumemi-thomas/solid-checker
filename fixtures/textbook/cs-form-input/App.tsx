import { createSignal } from "solid-js";

export function Controlled() {
  const [text, setText] = createSignal("");
  const [done, setDone] = createSignal(false);
  return (
    <form>
      <input value={text()} onInput={(event) => setText(event.currentTarget.value)} />
      <input type="checkbox" checked={done()} onChange={(event) => setDone(event.currentTarget.checked)} />
      <p>
        {text()} {done() ? "done" : "open"}
      </p>
    </form>
  );
}
