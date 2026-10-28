import * as S from "solid-js";

const [store] = S.createStore<{ v?: number }>({ v: 0 });

// Positive: the store created through a namespace import.
export function NamespaceStore() {
  delete store.v;
}
