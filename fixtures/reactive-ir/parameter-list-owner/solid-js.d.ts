declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
    div: {};
    p: {};
    span: {};
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T | ((previous: T) => T)) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function onSettled(callback: () => void): void;
  export function onCleanup(cleanup: () => void): void;
  export function createRoot<T>(init: (dispose: () => void) => T): T;
  export interface Context<T> { id: symbol; defaultValue: T }
  export function createContext<T>(defaultValue: T): Context<T>;
  export function useContext<T>(context: Context<T>): T;
}
