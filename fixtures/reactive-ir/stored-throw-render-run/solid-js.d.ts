declare namespace JSX {
  // `ref` is the published `Ref<T>` of `@solidjs/web` 2.0.0-rc.21 narrowed to the forms the cases use: a callback, an array of them, or nothing.
  type RefCallback = (element: HTMLElement) => void;
  interface IntrinsicElements {
    div: { children?: unknown };
    i: { children?: unknown };
    li: { children?: unknown };
    p: { ref?: RefCallback | RefCallback[] | undefined; children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Copied from the published 2.0.0-rc.21 typings, JSDoc dropped; `FlowProps` and `Element` simplified to the JSX the cases use.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  interface BaseEffectOptions {
    name?: string;
  }
  export interface EffectOptions extends BaseEffectOptions {
    defer?: boolean;
    transparent?: boolean;
  }
  export function Errored(props: {
    fallback: JSX.Element | ((err: () => unknown, reset: () => void) => JSX.Element);
    children: JSX.Element;
  }): JSX.Element;
  export function For<T>(props: {
    each: readonly T[] | undefined | null | false;
    fallback?: JSX.Element;
    children: (item: Accessor<T>, index: Accessor<number>) => JSX.Element;
  }): JSX.Element;
  export function Repeat(props: {
    count: number;
    fallback?: JSX.Element;
    children: (index: number) => JSX.Element;
  }): JSX.Element;
  export function Show<T>(props: {
    when: T | undefined | null | false;
    fallback?: JSX.Element;
    children: JSX.Element | ((item: Accessor<NonNullable<T>>) => JSX.Element);
  }): JSX.Element;
  export type ContextProviderComponent<T> = (props: { value: T; children: JSX.Element }) => JSX.Element;
  export interface Context<T> extends ContextProviderComponent<T> { id: symbol; defaultValue: T | undefined; }
  export function createContext<T>(defaultValue?: T, options?: EffectOptions): Context<T>;
  export function useContext<T>(context: Context<T>): T;
}
