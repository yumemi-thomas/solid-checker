import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const acrossModules = () => {
  setValue(1);
};
