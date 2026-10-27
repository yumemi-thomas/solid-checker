// Declarations transcribed from solid-js@2.0.0-rc.17 and
// @solidjs/signals@2.0.0-rc.17.
//
// Byte-faithful where a claim reads them: `runWithOwner`
// (`dist/types/core/core.d.ts:340`), `getOwner` (`dist/types/core/owner.d.ts:66`),
// `createOwner` (`:100`), `createRoot` (`:139`), `onCleanup`
// (`dist/types/signals.d.ts:47`) and `action` (`dist/types/core/action.d.ts:70`),
// up to the `export declare` -> `export` an ambient block forbids.
//
// Reduced, with no claim reading them back: `Owner` and `Root` to the
// interfaces' names (`dist/types/core/types.d.ts:153`, `:208`), `createSignal`
// to its value overload, and `createEffect` to its two-function shape
// (`dist/types/signals.d.ts:427`) whose apply returns `(() => void) | void`, as
// the package says.

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
    div: { children?: any };
  }
  interface Element {}
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
  export function getOwner(): Owner | null;
  export function createOwner(options?: {
    id?: string;
    transparent?: boolean;
  }): Root;
  export function createRoot<T>(
    init: ((dispose: () => void) => T) | (() => T),
    options?: { id?: string; transparent?: boolean }
  ): T;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export type Disposable = () => void;
  export function onCleanup(fn: Disposable): Disposable;
  export function action<Args extends any[], Y, R>(
    genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>
  ): (...args: Args) => Promise<R>;
}
