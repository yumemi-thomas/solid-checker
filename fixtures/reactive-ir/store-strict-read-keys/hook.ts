import { createStore } from "solid-js";

type Remote = { then: () => void; refetch: () => void; data: number };

// ADR 0287's forwarding proxy: every Get forwards to the store it wraps, so
// each key warns exactly as it would on the store itself.
export function useForwardedStore() {
  const [state] = createStore<Remote>({ then: () => {}, refetch: () => {}, data: 1 });
  return new Proxy(state, { get: (target, prop, receiver) => Reflect.get(target, prop, receiver) });
}
