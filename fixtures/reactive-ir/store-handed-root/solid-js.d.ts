declare module "solid-js" {
  // The published rc.21 declarations these cases use (`@solidjs/signals`
  // `types/store/types.d.ts`, `store.d.ts`, `utils.d.ts`; `solid-js`
  // `types/client/core.d.ts`).
  export type Store<T> = T;
  export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
  export function createStore<T extends Record<PropertyKey, any>>(
    initialValue: T,
    options?: { name?: string },
  ): [T, StoreSetter<T>];
  export function merge<T extends unknown[]>(...sources: T): T[number];
  export function omit<T extends Record<any, any>, K extends readonly (keyof T)[]>(
    props: T,
    ...keys: K
  ): Omit<T, K[number]>;
  export interface Context<T> {
    (props: { value: T; children?: any }): any;
    id: symbol;
    defaultValue: T;
  }
  export function createContext<T>(defaultValue?: T): Context<T>;
  export function useContext<T>(context: Context<T>): T;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
  export function createEffect<T>(compute: () => T, effect: (value: T) => void): void;
}
