// Declarations transcribed from solid-js@2.0.0-rc.21 and @solidjs/signals@2.0.0-rc.21 (the same reductions as
// `after-await-helper-callback`, plus `latest`, `isPending`, `onSettled`, `resolve` and `until`, each to the signature this
// fixture calls: `dist/types/core/core.d.ts`, `dist/types/signals.d.ts`).

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
    div: { children?: any };
    span: { children?: any };
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

declare module "solid-js" {
  export interface Owner {}
  export interface Root extends Owner {}
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export function createTrackedEffect(
    compute: () => void | (() => void),
    options?: object
  ): void;
  export function createMemo<T>(compute: (prev: T | undefined) => T | Promise<T>): Accessor<T>;
  export function untrack<T>(fn: () => T, label?: string | false): T;
  export function latest<T>(fn: () => T): T;
  export function isPending(fn: () => unknown): boolean;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export function getOwner(): Owner | null;
  export function createRoot<T>(
    init: ((dispose: () => void) => T) | (() => T),
    options?: { id?: string; transparent?: boolean }
  ): T;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export type Disposable = () => void;
  export function onCleanup(fn: Disposable): Disposable;
  export function onSettled(fn: () => void | (() => void)): void;
  export function resolve<T>(fn: () => T): Promise<T>;
  export function until<T>(fn: () => T): Promise<T>;
  export function action<Args extends any[], Y, R>(
    genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>
  ): (...args: Args) => Promise<R>;
}
