declare namespace JSX {
  type EventHandler<T, E> = (event: E & { currentTarget: T; target: Element }) => void;
  interface IntrinsicElements {
    button: { onClick?: EventHandler<HTMLButtonElement, MouseEvent>; children?: unknown; [attribute: string]: unknown };
    div: { children?: unknown };
    p: { children?: unknown };
  }
  type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  type Element = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  interface ArrayElement extends Array<Element> {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Declarations transcribed from @solidjs/signals@2.0.0-rc.21, which solid-js@2.0.0-rc.21 re-exports: `flush` `dist/types/core/scheduler.d.ts:448-449`, `action`
// `dist/types/core/action.d.ts:78`, `createTrackedEffect` `dist/types/signals.d.ts`, `untrack`; `Component` from the published `solid-js` types.
declare module "solid-js" {
  export type Component<P extends Record<string, any> = {}> = (props: P) => JSX.Element;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createTrackedEffect(compute: () => void | (() => void)): void;
  export function untrack<T>(fn: () => T): T;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
}
