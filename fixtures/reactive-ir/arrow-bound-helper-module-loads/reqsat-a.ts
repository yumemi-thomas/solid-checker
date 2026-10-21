// Negative: `require satisfies X` is a direct call.
declare function require(module: string): unknown;
(require satisfies ((module: string) => unknown))("./reqsat-b");
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
