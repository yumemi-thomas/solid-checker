declare namespace JSX {
  interface IntrinsicElements {
    div: { title?: unknown };
    button: { onClick?: unknown };
    span: { title?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createMemo<T>(fn: () => Promise<T>): () => T;
  export function createMemo<T>(fn: () => T): () => T;
  // 2.0 folds the store APIs into core.
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
  export function onSettled(callback: () => void): void;
  // `merge`, `omit` and their types, copied from `@solidjs/signals@2.0.0-rc.13`
  // `dist/types/store/utils.d.ts` (JSDoc and `declare`, which an ambient module
  // cannot repeat, dropped).
  type DistributeOverride<T, F> = T extends undefined ? F : T;
  type Override<T, U> = T extends any ? U extends any ? {
      [K in keyof T]: K extends keyof U ? DistributeOverride<U[K], T[K]> : T[K];
  } & {
      [K in keyof U]: K extends keyof T ? DistributeOverride<U[K], T[K]> : U[K];
  } : T & U : T & U;
  type OverrideSpread<T, U> = T extends any ? {
      [K in keyof ({
          [K in keyof T]: any;
      } & {
          [K in keyof U]?: any;
      } & {
          [K in U extends any ? keyof U : keyof U]?: any;
      })]: K extends keyof T ? Exclude<U extends any ? U[K & keyof U] : never, undefined> | T[K] : U extends any ? U[K & keyof U] : never;
  } : T & U;
  type Simplify<T> = T extends any ? {
      [K in keyof T]: T[K];
  } : T;
  type _Merge<T extends unknown[], Curr = {}> = T extends [
      infer Next | (() => infer Next),
      ...infer Rest
  ] ? _Merge<Rest, Override<Curr, Next>> : T extends [...infer Rest, infer Next | (() => infer Next)] ? Override<_Merge<Rest, Curr>, Next> : T extends [] ? Curr : T extends (infer I | (() => infer I))[] ? OverrideSpread<Curr, I> : Curr;
  export type Merge<T extends unknown[]> = Simplify<_Merge<T>>;
  export function merge<T extends unknown[]>(...sources: T): Merge<T>;
}
