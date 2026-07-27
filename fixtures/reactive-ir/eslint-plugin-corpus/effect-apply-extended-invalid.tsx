import {
  createEffect,
  createSignal,
  createStore,
  type Accessor,
} from "solid-js";

const [count] = createSignal(0);
const applyCount = () => console.log(count());
createEffect(() => count(), applyCount);

export function install(options: { read: Accessor<number> }) {
  createEffect(() => 1, () => options.read());
}

const [store] = createStore({ user: { name: "Ada" } });
createEffect(() => store, current => console.log(current.user.name));

const bundledEffect = () => count();
const bundledError = () => count();
createEffect(() => count(), {
  effect: bundledEffect,
  error: bundledError,
});
