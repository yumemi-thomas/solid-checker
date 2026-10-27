declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void; children?: unknown };
    i: { children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Copied from the published solid-js 2.0.0-rc.17 typings, JSDoc dropped:
// `RenderedElement`, `Element` and `ArrayElement` from `types/types.d.ts`,
// `Errored` and `Loading` from `types/client/flow.d.ts` (`ErrorAccessor` is
// its local alias), `Accessor` and `runWithOwner` from `@solidjs/signals`
// `dist/types`. `Owner` is opaque here: only `null` is passed.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export interface Owner {}
  export type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  export type Element = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  export interface ArrayElement extends Array<Element> {}
  type ErrorAccessor = Accessor<unknown>;
  export function Errored(props: {
    fallback: Element | ((err: ErrorAccessor, reset: () => void) => Element);
    children: Element;
  }): Element;
  export function Loading(props: {
    fallback?: Element;
    on?: any;
    children: Element;
  }): Element;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
}
