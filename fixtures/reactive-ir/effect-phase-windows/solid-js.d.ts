// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls: `createSignal` to its value overload
// (`dist/types/signals.d.ts:302`), `createMemo` to its plain overload (`:358`),
// `createEffect` (`:427`) and `createRenderEffect` (`:457`) to a compute and an
// effect function, `createReaction` (`:530`) to its function form, `getOwner`
// and `runWithOwner` (`dist/types/core/owner.d.ts`), and `flush`
// (`dist/types/core/scheduler.d.ts:384-385`).

declare namespace JSX {
  interface IntrinsicElements {
    button: { children?: any; onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export type Owner = object;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T): Accessor<T>;
  export function createEffect<const T>(compute: (prev: T | undefined) => T, effect: (value: T, prev?: T) => void | (() => void)): void;
  export function createRenderEffect<const T>(compute: (prev: T | undefined) => T, effect: (value: T, prev?: T) => void | (() => void)): void;
  export function createReaction(effectFn: () => void | (() => void)): (tracking: () => void) => void;
  export function getOwner(): Owner | null;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
}
