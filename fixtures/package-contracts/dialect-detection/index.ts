import {
  createComponent,
  createContext,
  createMemo,
  createResource,
  on,
  useContext
} from "solid-js";
import { createStore } from "solid-js/store";

export function observe(source: () => unknown): () => unknown {
  return on(source, value => value);
}

function tracked(callback: () => unknown): () => unknown {
  return createMemo(callback);
}

export function indirect(source: () => unknown): () => unknown {
  return tracked(() => source());
}

function callNow(callback: () => unknown): unknown {
  return callback();
}

export function indirectResource(source: () => unknown): void {
  createResource(() => callNow(source), value => value);
}

export function returnedAccessor(): () => number {
  const source = createMemo(() => 1);
  const wrapper = () => source();
  return wrapper;
}

export function returnedResource(): () => unknown {
  const [resource] = createResource(() => 1, value => value);
  const wrapper = () => resource();
  return wrapper;
}

export function assignedResource(): () => unknown {
  let resource!: () => unknown;
  [resource] = createResource(() => 1, value => value);
  return resource;
}

export function tupleResult() {
  const [store] = createStore({ value: 1 });
  const accessor = createMemo(() => store.value);
  return [store, accessor] as const;
}

export function objectResult() {
  const state = createMemo(() => true);
  return {
    active: () => state(),
    pending: createMemo(() => !state())
  };
}

export function projectedObjectResult() {
  return objectResult().pending;
}

export function projectedAliasResult() {
  const result = objectResult();
  return result.active;
}

export function projectedTupleResult() {
  const result = tupleResult();
  return result[1];
}

export function identityResult<T>(value: T): T {
  return value;
}

type RouterState = {
  location: { pathname: () => string };
  params: Record<string, string>;
};

const RouterContext = createContext<RouterState>();

function createMemoObject<T extends Record<string, unknown>>(read: () => T): T {
  return new Proxy({} as T, {
    get(_target, property) {
      return createMemo(() => read()[property as string])();
    }
  });
}

function createRouterState(): RouterState {
  const pathname = createMemo(() => "/users/1");
  const location = { pathname };
  const params = createMemoObject(() => ({ id: "1" }));
  return { location, params };
}

export function mountRouter(): unknown {
  const routerState = createRouterState();
  return createComponent(RouterContext.Provider, { value: routerState });
}

function useRouter(): RouterState {
  return identityResult(useContext(RouterContext));
}

export function contextLocation() {
  return useRouter().location;
}

export function contextParams() {
  return useRouter().params;
}
