// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls: `createSignal` to its value overload
// (`dist/types/signals.d.ts:302`), `EffectFunction` (`:75`), `createReaction`
// (`:530`), `createRoot` (`dist/types/core/owner.d.ts:139`).

declare namespace JSX {
  interface IntrinsicElements {
    button: { children?: any; onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createReaction(effectFn: EffectFunction<undefined>): (tracking: () => void) => void;
  export function createRoot<T>(init: (() => T) | ((dispose: () => void) => T)): T;
}
