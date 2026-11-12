import { createEffect, createSignal, flush } from "solid-js";
const [k, setK] = createSignal(0);
let Late: undefined | (new () => object);
export function App() {
  // A field initializer runs when an instance is constructed, not where the class is written: here in an effect's apply.
  setTimeout(() => {
    Late = class {
      x = flush();
    };
    setK(1);
  });
  createEffect(
    () => k(),
    () => {
      if (Late) new Late();
    },
  );
  return <p>x</p>;
}
