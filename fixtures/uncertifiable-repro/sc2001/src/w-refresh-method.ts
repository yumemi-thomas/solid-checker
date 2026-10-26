// hoxxes-briefing create-cached-query.ts:127-129: a method of the object a factory returns calls `refresh()`, which starts a write
// transaction; the consumer hands the result to `merge`.
import { createMemo, createSignal, merge, refresh } from "solid-js";

export function createCachedQuery(key: () => string) {
  const [version] = createSignal(0);
  const state = createMemo(() => `${key()}:${version()}`);
  return {
    get data() {
      return state();
    },
    refresh() {
      refresh(state);
    },
  };
}

export function useCached(props: { id: string }) {
  return merge(props, createCachedQuery(() => props.id));
}
