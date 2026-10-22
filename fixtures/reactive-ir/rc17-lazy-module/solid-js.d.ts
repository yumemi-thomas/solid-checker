// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17.
//
// Byte-faithful, because every claim in this fixture rests on them, up to the
// `export declare` an ambient `declare module` block spells `export`:
//
// - `lazyModule`   solid-js `types/client/component.d.ts:153-157` (re-exported
//                  by `types/index.d.ts:5`)
//
// Reduced, and safe to reduce because no claim reads them back: `createSignal`
// keeps only its value overload and `createEffect` its two-function shape
// (rc.17 `dist/types/signals.d.ts:302` and `:427`); and `JSX` itself.

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export function lazyModule<T extends Record<string, any>>(fn: () => Promise<T>, moduleUrl?: string): (() => Promise<T>) & {
      preload: () => Promise<T>;
      peek: () => T | undefined;
      moduleUrl?: string;
  };
}
