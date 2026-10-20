import { createSignal, createMemo, createEffect, onCleanup } from "solid-js";

export function AsyncInline() {
  const data = createMemo(async () => 1);
  return <button onClick={() => console.log(data())}>c</button>;
}
export function OwnerInline() {
  const [count] = createSignal(0);
  return (
    <button
      onClick={() => {
        createEffect(() => count(), () => undefined);
        onCleanup(() => undefined);
      }}
    >
      b
    </button>
  );
}
