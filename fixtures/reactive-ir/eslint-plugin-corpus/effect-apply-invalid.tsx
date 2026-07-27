import {
  createEffect,
  createMemo,
  createRenderEffect,
  createSignal,
  type Accessor,
} from "solid-js";

const [count] = createSignal(0);
const countAlias = count;
createEffect(() => count(), () => countAlias());

const doubled = createMemo(() => count() * 2);
createRenderEffect(() => doubled(), () => doubled());

function install(read: Accessor<number>) {
  createEffect(() => 1, () => read());
}
export { install };
