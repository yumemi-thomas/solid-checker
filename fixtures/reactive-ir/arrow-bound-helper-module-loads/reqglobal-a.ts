// Negative: the global `require` (no shim), wrapped; the visitor records the load.
(require as (module: string) => unknown)("./reqglobal-b");
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
