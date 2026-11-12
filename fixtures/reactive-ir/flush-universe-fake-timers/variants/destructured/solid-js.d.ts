declare namespace JSX {
  type EventHandler<T, E> = (event: E & { currentTarget: T; target: Element }) => void;
  interface IntrinsicElements {
    button: { onClick?: EventHandler<HTMLButtonElement, MouseEvent>; children?: unknown };
    div: { children?: unknown };
  }
}

// Declarations transcribed from @solidjs/signals@2.0.0-rc.21, which solid-js@2.0.0-rc.21 re-exports: `flush` `dist/types/core/scheduler.d.ts:448-449`, `action`
// `dist/types/core/action.d.ts:78`, `createTrackedEffect` `dist/types/signals.d.ts`.
declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createTrackedEffect(compute: () => void | (() => void)): void;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
