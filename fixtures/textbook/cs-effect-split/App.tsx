import { createEffect, createSignal } from "solid-js";

export function TitleSync() {
  const [name, setName] = createSignal("guest");
  createEffect(
    () => name(),
    (value) => {
      document.title = `Hello ${value}`;
      return () => {
        document.title = "";
      };
    },
  );
  return <input value={name()} onInput={(event) => setName(event.currentTarget.value)} />;
}
