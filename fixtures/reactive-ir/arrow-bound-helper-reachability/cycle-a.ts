import { createSignal } from "solid-js";
import { CycleCaller } from "./cycle-b";

const [, setCycled] = createSignal(0);

export const cycled = () => {
  setCycled(1);
};

export const keep = CycleCaller;
