import { createSignal, lazy, onCleanup } from "solid-js";

const [crossFileLazySource] = createSignal(0);

export const CrossFileLazy = lazy(async () => {
  crossFileLazySource();
  onCleanup(() => {});
  return { default: () => <div /> };
});
