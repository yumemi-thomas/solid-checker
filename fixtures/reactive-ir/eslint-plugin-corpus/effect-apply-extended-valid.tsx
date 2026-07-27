import {
  createEffect,
  createSignal,
  untrack,
} from "solid-js";

const [count, setCount] = createSignal(0);

const applyLater = () => {
  const click = () => console.log(count());
  return click;
};
createEffect(() => count(), applyLater);

createEffect(() => count(), () => untrack(() => count()));

createEffect(() => count(), {
  effect: value => setCount(value),
  error: () => setCount(0),
});

const applyCount = (value: number) => setCount(value);
const handleError = () => setCount(0);
createEffect(() => count(), {
  effect: applyCount,
  error: handleError,
});
