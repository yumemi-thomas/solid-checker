import { createSignal, createStore } from "solid-js";

declare function sink(value: unknown): void;
declare const server: boolean;

type Query = { search: string; isError: boolean; error: unknown };

// The `@tanstack/solid-query` 6.0.0-rc.0 `useBaseQuery` shape, without its
// `isServer` branch: a symbol and a pass-through set forward, an error throws,
// and every other read forwards (ADR 0287).
export function useForwardingProxy() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  const passthrough = new Set(["error", "isError"]);
  return new Proxy(state, {
    get(target, prop, receiver) {
      if (typeof prop === "symbol") {
        return Reflect.get(target, prop, receiver);
      }
      if (passthrough.has(prop)) {
        return Reflect.get(target, prop, receiver);
      }
      if (state.isError) {
        throw state.error;
      }
      return Reflect.get(target, prop, receiver);
    },
  });
}

// Two-parameter and concise-arrow forwards.
export function useTwoParameterProxy() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, {
    get(target, prop) {
      return Reflect.get(target, prop);
    },
  });
}
export function useArrowProxy() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, { get: (target, prop, receiver) => Reflect.get(target, prop, receiver) });
}

// `useQuery` forwarding to `useBaseQuery` in the same file.
export function useForwarded() {
  return useForwardingProxy();
}

// Refusals.
export function useSetTrap() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, {
    get(target, prop, receiver) {
      return Reflect.get(target, prop, receiver);
    },
    set() {
      return true;
    },
  });
}
export function useHasTrap() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, {
    get(target, prop, receiver) {
      return Reflect.get(target, prop, receiver);
    },
    has() {
      return true;
    },
  });
}
// The server branch of `useBaseQuery`, guarded by a condition no host fold
// decides: it returns another object's member.
export function useOtherReturn() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  const resolved = { search: "resolved" };
  return new Proxy(state, {
    get(target, prop, receiver) {
      if (server && prop in resolved) {
        return Reflect.get(resolved, prop);
      }
      return Reflect.get(target, prop, receiver);
    },
  });
}
// Another store's member: still a store read, but not this proof's.
export function useOtherStore() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  const [other] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, {
    get(_target, prop) {
      return Reflect.get(other, prop);
    },
  });
}
export function useWrittenTarget() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  const plain: Query = { search: "", isError: false, error: null };
  return new Proxy(state, {
    get(target, prop, receiver) {
      target = plain;
      return Reflect.get(target, prop, receiver);
    },
  });
}
export function useLeakedTarget() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, {
    get(target, prop, receiver) {
      sink(target);
      return Reflect.get(target, prop, receiver);
    },
  });
}
export function useFallthrough() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, {
    get(target, prop, receiver) {
      if (prop === "search") {
        return Reflect.get(target, prop, receiver);
      }
      return undefined;
    },
  });
}
export function usePlainTarget() {
  const plain: Query = { search: "", isError: false, error: null };
  return new Proxy(plain, {
    get(target, prop, receiver) {
      return Reflect.get(target, prop, receiver);
    },
  });
}
export function useSignalTarget() {
  const [search] = createSignal("");
  return new Proxy(search, {
    get(target, prop, receiver) {
      return Reflect.get(target, prop, receiver);
    },
  });
}
const handlers: ProxyHandler<Query> = {
  get(target, prop, receiver) {
    return Reflect.get(target, prop, receiver);
  },
};
export function useSpreadHandler() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, { ...handlers });
}
export function useHeldHandler() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  return new Proxy(state, handlers);
}
// The root also escapes outside the proxy.
export function useEscapedRoot() {
  const [state] = createStore<Query>({ search: "", isError: false, error: null });
  sink(state);
  return new Proxy(state, {
    get(target, prop, receiver) {
      return Reflect.get(target, prop, receiver);
    },
  });
}
// Two forwards to different sources.
export function useForwardedEither(flag: boolean) {
  if (flag) {
    return useForwardingProxy();
  }
  return useTwoParameterProxy();
}
