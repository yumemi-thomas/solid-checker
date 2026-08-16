const state = () => 1;

import { createMemo } from "solid-js";

export const getterObject = {
  get value() {
    return state();
  }
};

export const getterFunction = Object.defineProperty(() => 1, "reactive", {
  get() {
    return state;
  }
});

export function getterResult() {
  const reactive = createMemo(() => 1);
  return {
    get value() {
      return reactive();
    }
  };
}
