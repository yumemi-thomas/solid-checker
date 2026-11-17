// Declarations transcribed from solid-js@2.0.0-rc.21 and @solidjs/signals@2.0.0-rc.21, as `latent-silent-arms` and
// `rc21-action-unplaced` carry them: `action` (`dist/types/core/action.d.ts:78`) byte-faithful but for `declare`;
// `createSignal` reduced to its value overload and `createMemo` (`signals.d.ts:358`) to the overload this fixture calls.
// `JSX` is this file's, reduced to the elements the fixture renders.
declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
    i: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T): Accessor<T>;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
