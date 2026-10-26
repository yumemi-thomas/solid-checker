// app-game solid-three core/store.ts:319-322: a member of the object handed to createStore reads the store it fills.
import { createStore } from "solid-js";

export function createViewport() {
  const [store] = createStore({
    dpr: 1,
    scaled: (width: number) => width * store.dpr,
  });
  return store;
}
