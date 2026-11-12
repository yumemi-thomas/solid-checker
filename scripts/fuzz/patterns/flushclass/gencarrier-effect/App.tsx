import { createEffect, createSignal, flush } from "solid-js";
const [k, setK] = createSignal(0);
let late: undefined | Generator<void, void, unknown>[];
export function App() {
  // `map` only creates the generator object: the body runs at `.next()`, on whatever stack calls it.
  setTimeout(() => {
    late = [1].map(function* () {
      flush();
    });
    setK(1);
  });
  createEffect(
    () => k(),
    () => {
      late?.[0]?.next();
    },
  );
  return <p>x</p>;
}
