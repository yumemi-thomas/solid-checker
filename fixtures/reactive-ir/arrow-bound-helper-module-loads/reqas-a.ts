// Negative: a TypeScript wrapper erases at run time; this still loads reqas-b.
declare function require(module: string): unknown;
(require as (module: string) => unknown)("./reqas-b");
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
