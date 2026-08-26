import {
  createComponent,
  createContext,
  createEffect,
  createMemo,
  createResource,
  on,
  createRoot,
  runWithOwner,
  type Owner,
  useContext
} from "solid-js";
import { createStore } from "solid-js/store";
import { minifiedExportObject as exportObject } from "./namespace-helper";

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

export function constructionCandidates(
  absent: null | undefined,
  items: unknown[],
  map: Map<unknown, unknown>,
  set: Set<unknown>,
  mode: "open" | "closed" | 0 | 1 | false | true
): void {
  void absent;
  void items;
  void map;
  void set;
  void mode;
}

// One identity-looking branch is not an identity contract. The generic probe
// supplies a fresh object here and observes `false` on the other path; package
// generation must therefore omit `returns` instead of claiming `argument[0]`.
export function conditionalIdentity<T>(value: T, keep: boolean): T | false {
  if (keep) return value;
  return false;
}

export function isObject(value: unknown): boolean {
  return typeof value === "object";
}

export function guardedIdentity<T>(value: T, keep: boolean): T | undefined {
  if (keep) return value;
}

const conditionalCallbackAdapter = (fn: (value?: number) => unknown): (() => unknown) =>
  fn.length ? () => fn(1) : fn;

export function memoThroughConditionalAdapter(
  fn: (value?: number) => unknown
): () => unknown {
  return createMemo(conditionalCallbackAdapter(fn));
}

export function conditionalInlineCallback(
  fn: (done: () => void) => void,
  owner?: Owner
): void {
  createRoot(() => {
    (owner ? done => runWithOwner(owner, () => fn(done)) : fn)(() => {});
  });
}

const accessMaybe = (value: number | (() => number)): number =>
  typeof value === "function" ? value() : value;

export function effectThroughMaybeAccessor(value: number | (() => number)): void {
  createEffect(() => accessMaybe(value));
}

function minifiedFunctionAlias(): void {}

// Bundlers commonly spell namespace exports as a call whose callee has a
// short/minified name. The call result is the exported value; its span must
// never inherit the callee's function summary.
const namespaceExport = exportObject({ member: minifiedFunctionAlias });
export { namespaceExport as t };

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
