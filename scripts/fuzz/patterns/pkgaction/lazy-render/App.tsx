import { Loading, Show, createSignal, lazy } from "solid-js";
const Lazy = lazy(() => import("fz-lazy"));
const [shown, show] = createSignal(false);
export async function after() {
  await Lazy.preload();
  show(true);
}
export function App() {
  return (
    <Loading fallback={null}>
      <Show when={shown()}>
        <Lazy />
      </Show>
    </Loading>
  );
}
