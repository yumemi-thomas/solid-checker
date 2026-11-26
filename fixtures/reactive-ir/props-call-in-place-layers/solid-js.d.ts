declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown; onClick?: () => void } }
  interface Element {}
}

// solid-js@2.0.0-rc.21: `createSignal` and `omit` (re-exported from
// @solidjs/signals@2.0.0-rc.21, `dist/types/store/utils.d.ts` 257-261),
// narrowed to the overloads this fixture calls.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => void];
  export type Omit<T, K extends readonly (keyof T)[]> = {
      [P in keyof T as Exclude<P, K[number]>]: T[P];
  };
  export function omit<T extends Record<any, any>, K extends readonly (keyof T)[]>(props: T, ...keys: K): Omit<T, K>;
}
