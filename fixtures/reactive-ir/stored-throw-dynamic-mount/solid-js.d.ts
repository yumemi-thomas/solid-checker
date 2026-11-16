// Stubs of the published 2.0.0-rc.21 signatures this fixture uses: `createContext`, `useContext`, `createTrackedEffect`, `flush`, `Errored`
// (`solid-js` and `@solidjs/signals` types), and `@solidjs/web`'s `dynamic` (`types/index.d.ts:157`: a `Component` whatever the source returns).
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    p: { children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute {
    children: {};
  }
}

declare module "solid-js" {
  export type Component<P = {}> = (props: P) => JSX.Element;
  export interface Context<T> {
    id: symbol;
    defaultValue: T;
  }
  export function createContext<T>(defaultValue?: T, description?: string): Context<T>;
  export function useContext<T>(context: Context<T>): T;
  export function createTrackedEffect(compute: () => void | (() => void)): void;
  export function flush(): void;
  export function Errored(props: { fallback: JSX.Element | ((err: () => unknown, reset: () => void) => JSX.Element); children: JSX.Element }): JSX.Element;
  export interface DynamicOptions {
    static?: boolean;
  }
}

declare module "@solidjs/web" {
  import type { Component, DynamicOptions } from "solid-js";
  export function dynamic<T>(source: () => T | Promise<T> | null | undefined | false, options?: DynamicOptions): Component<{}>;
}
