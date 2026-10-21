// Negative: `require!(...)` is a direct call.
declare function require(module: string): unknown;
require!("./reqnn-b");
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
