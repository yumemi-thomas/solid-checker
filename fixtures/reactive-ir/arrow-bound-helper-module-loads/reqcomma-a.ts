// Negative: `(0, require)(...)` is a call of `require` the load facts do not
// record; the module may be on a cycle.
declare function require(module: string): unknown;
(0, require)("./reqcomma-b");
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
