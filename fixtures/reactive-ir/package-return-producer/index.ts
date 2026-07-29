import { createMemo, createSignal, createStore } from "solid-js";

export const packageVersion = "1.0.0";
export const firstConstant = 1,
  /** Secondary value. */
  secondConstant = 2;

/**
 * Example only:
 * import { createEffect } from "solid-js";
 * export function Ghost() { return null; }
 * const handleClick = (event: Event) => { console.log(event); };
 */

export function createCount() {
  const [count] = createSignal<number>(0);
  // Preserve the accessor's reactive identity across the package boundary.
  return count;
}

export { createCount as createAliasedCount };

export const createArrowCount = (): (() => number) => {
  const [count] = createSignal(0);
  return count;
};

export const createMemoCount = (): (() => number) => {
  return createMemo(() => 1);
};

export function createWrappedCount(compute: () => number): () => number {
  return createMemo(compute);
}

function memoize(compute: () => number): () => number {
  return createMemo(compute);
}

export function createTransitivelyWrapped(compute: () => number): () => number {
  return memoize(compute);
}

export function createState() {
  const [state] = createStore({ value: 1 });
  return state;
}

export function identityFactory(): <T>(
  value: T,
) => T {
  return value => value;
}

export function nestedGeneric<T extends Record<string, Array<number>>>(value: T): T {
  return value;
}

export function callbackGeneric<T extends (...args: unknown[]) => void>(callback: T): T {
  return callback;
}

export async function loadValue(): Promise<number> {
  return 1;
}

export function listen(
  targets: EventTarget[],
  handler: (event: Event) => void,
): void {
  const apply = (): void => {
    for (const target of targets) {
      target.addEventListener("value", handler);
    }
  };
  apply();
}

type DeferredMethod<T> = {
  install: (callback: (left: T, right: T) => number) => void;
  activate: () => void;
};

export function configureDeferredMethod<T>(
  target: DeferredMethod<T>,
  callback: (left: T, right: T) => number,
): DeferredMethod<T> {
  target.activate = () => target.install(callback);
  return target;
}

class RetainedProxyHandler {
  constructor(private readonly callback: () => void) {}

  get(): void {
    this.callback();
  }
}

export function createDeferredProxy(callback: () => void): object {
  return new Proxy({}, new RetainedProxyHandler(callback));
}
