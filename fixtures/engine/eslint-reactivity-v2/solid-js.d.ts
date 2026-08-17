declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => void;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function createMemo<T>(compute: () => T): Accessor<T>;
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
  // Byte-faithful to `@solidjs/signals@2.0.0-rc.0`'s
  // `createTrackedEffect(compute: () => void | (() => void), options?)`.
  export function createTrackedEffect(callback: () => void | (() => void)): void;
  export function onCleanup(callback: () => void): void;
  export function flush(): void;
  export function refresh<T>(target: Accessor<T>): void;
  export function action<T>(callback: (...args: never[]) => T): () => T;
  export function merge<A, B>(first: A, second: B): A & B;
  export function omit<T extends object, K extends keyof T>(value: T, ...keys: K[]): Omit<T, K>;
}

declare module "@solidjs/web" {
  export function render(code: () => unknown, element: object): () => void;
}
