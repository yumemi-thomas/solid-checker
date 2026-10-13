// Declarations transcribed from solid-js@2.0.0-rc.9, which re-exports each of
// these names from @solidjs/signals@2.0.0-rc.9 (`types/index.d.ts:1`).
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - both `omit` overloads   @solidjs/signals `dist/types/store/utils.d.ts:250-251`
//                           (the second, the predicate form, is new in rc.9)
// - the `Omit` they return  `dist/types/store/utils.d.ts:216-218`
// - `untrack`               @solidjs/signals `dist/types/core/core.d.ts:105`
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files spell `export declare function`.)
//
// Reduced, and safe to reduce because no claim reads them back: `createSignal`
// keeps only its value overload (`dist/types/signals.d.ts:263`) without the
// `Setter` inference machinery, and `JSX` is this file's.

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any; title?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  export type Omit<T, K extends readonly (keyof T)[]> = {
      [P in keyof T as Exclude<P, K[number]>]: T[P];
  };
  export function omit<T extends Record<any, any>, K extends readonly (keyof T)[]>(props: T, ...keys: K): Omit<T, K>;
  export function omit<T extends Record<any, any>>(props: T, hidden: (key: keyof T & (string | symbol)) => boolean): Partial<T>;
}
