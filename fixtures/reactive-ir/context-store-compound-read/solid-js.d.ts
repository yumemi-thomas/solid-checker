declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void; children?: unknown };
    i: {};
    p: { children?: unknown; title?: string };
  }
  interface Element {}
  // As `solid-js`'s JSX namespace declares it: an element's children are its `children` prop.
  interface ElementChildrenAttribute {
    children: {};
  }
}

declare module "solid-js" {
  // Reduced from @solidjs/signals rc.21: `Store<T> = T`, the plain `createStore` overload and `StoreSetter`; `merge` and `omit`
  // with their types as `dist/types/store/utils.d.ts` writes them.
  export type StoreSetter<T> = (fn: (draft: T) => void) => void;
  export function createStore<T extends object>(value: T): [T, StoreSetter<T>];
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
  export type Omit<T, K extends readonly (keyof T)[]> = {
      [P in keyof T as Exclude<P, K[number]>]: T[P];
  };
  export function omit<T extends Record<any, any>, K extends readonly (keyof T)[]>(props: T, ...keys: K): Omit<T, K>;
  // `solid-js` rc.21 `types/client/core.d.ts`: a context is its own provider component (`FlowComponent<{ value: T }>`).
  export type ContextProviderComponent<T> = (props: { value: T; children: JSX.Element }) => JSX.Element;
  export interface Context<T> extends ContextProviderComponent<T> {
    id: symbol;
    defaultValue: T | undefined;
  }
  export function createContext<T>(defaultValue?: T): Context<T>;
  export function useContext<T>(context: Context<T>): T;
}
