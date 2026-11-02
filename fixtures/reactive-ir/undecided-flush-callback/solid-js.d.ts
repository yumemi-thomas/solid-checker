// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls: `createSignal` to its value overload
// (`dist/types/signals.d.ts:302`), `flush` (`dist/types/core/scheduler.d.ts:384-385`)
// and `untrack` (`dist/types/core/core.d.ts:167`).

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
