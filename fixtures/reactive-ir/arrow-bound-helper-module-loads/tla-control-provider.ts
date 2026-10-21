import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
