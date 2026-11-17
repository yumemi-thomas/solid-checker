// Stubs of the published 2.0.0-rc.21 signatures this fixture uses, byte-faithful but for `declare`: `createContext`, `useContext`
// (`solid-js` `types/client/core.d.ts:92, 125`; `Context` reduced to the fields read), `createMemo` (`@solidjs/signals`
// `dist/types/signals.d.ts:358`, `ComputeFunction` and `MemoOptions` reduced to the overload this fixture calls), `createRoot`
// (`dist/types/core/owner.d.ts:139`), `untrack` (`core.d.ts:204`) and `runWithOwner` (`core.d.ts:388`).
declare namespace JSX {
  interface IntrinsicElements { div: { children?: unknown }; p: { children?: unknown }; button: { onClick?: () => void; children?: unknown } }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}
declare module "solid-js" {
  export interface Owner {}
  export interface Context<T> { id: symbol; defaultValue: T | undefined }
  export function createContext<T>(defaultValue?: T, options?: object): Context<T>;
  export function useContext<T>(context: Context<T>): T;
  export function createMemo<T>(compute: (prev: T | undefined) => T, options?: object): () => T;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: { id?: string; transparent?: boolean }): T;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
}
