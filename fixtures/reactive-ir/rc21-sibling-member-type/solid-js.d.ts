declare namespace JSX {
  interface IntrinsicElements { p: {}; i: {} }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export interface Context<T> { (props: { value: T; children?: unknown }): unknown }
  export function createContext<T>(value: T): Context<T>;
  export function useContext<T>(context: Context<T>): T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => void];
  export function createMemo<T>(fn: () => T): Accessor<T>;
}

declare module "@solidjs/web" {
  export function render(fn: () => unknown, el: unknown): void;
}
