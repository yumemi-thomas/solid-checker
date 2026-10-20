import b = require("./impeq-b");
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
export const keep = b;
