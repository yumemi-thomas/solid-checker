import {
  createEffect,
  createSignal,
  untrack,
} from "solid-js";

const [count] = createSignal(0);
createEffect(
  () => count(),
  value => console.log(value),
);
createEffect(
  () => count(),
  () => {
    untrack(() => count());
  },
);
createEffect(
  () => count(),
  () => {
    const later = () => count();
    return later;
  },
);

function install(read: () => number) {
  createEffect(() => 1, () => read());
}
export { install };
