import { createStore } from "solid-js";

// The angle-bracket assertion is a .ts-only transparent wrapper.
export function angleOwnSetter() {
  const [store, setStore] = createStore({ open: false });
  (<typeof setStore>setStore)(() => { store.open = true; });
  return store;
}
