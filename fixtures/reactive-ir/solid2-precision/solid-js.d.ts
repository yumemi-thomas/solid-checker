declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: unknown; children?: unknown };
    div: { children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  // Byte-faithful to `@solidjs/signals@2.0.0-rc.0`'s
  // `EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void`
  // and `onSettled(callback: () => void | (() => void))`. Do not loosen these:
  // a stub that returns `unknown` manufactures cleanup-return defects no real
  // project can produce, which is how the removed SC3004/SC9002 pair survived
  // for a full cycle (docs/precision-backlog.md).
  export function createEffect<T>(
    compute: () => T,
    apply: (value: T, previous?: T) => (() => void) | void,
  ): void;
  export function createMemo<T>(compute: () => T): () => T;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
  export function onCleanup(callback: () => void): void;
  export function onSettled(callback: () => void | (() => void)): void;
}
