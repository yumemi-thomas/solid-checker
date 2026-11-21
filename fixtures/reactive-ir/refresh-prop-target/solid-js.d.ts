declare namespace JSX {
  interface IntrinsicElements { div: {}; p: { children?: unknown }; button: { onClick?: (event: MouseEvent) => void; children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
  export function createMemo<T>(compute: () => T): Accessor<T>;
  export function createEffect<T>(compute: () => T, apply: (value: T, previous?: T) => (() => void) | void): void;
  // `@solidjs/signals@2.0.0-rc.21`: `refresh<T>(target: Refreshable<T>): Promise<T>`; an accessor stands for the brand here.
  export type SourceAccessor<T> = Accessor<T>;
  export function refresh<T>(target: Accessor<T>): Promise<T>;
}
