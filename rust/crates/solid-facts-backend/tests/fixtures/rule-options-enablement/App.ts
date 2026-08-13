import { createEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

count();

createEffect(() => {
  setCount(count() + 1);
});
