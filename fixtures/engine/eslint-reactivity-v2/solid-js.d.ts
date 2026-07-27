declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => void;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function createMemo<T>(compute: () => T): Accessor<T>;
  export function createEffect<T>(compute: () => T, apply: (value: T) => unknown): void;
  export function createTrackedEffect(callback: () => unknown): void;
  export function onCleanup(callback: () => void): void;
  export function flush(): void;
  export function refresh<T>(target: Accessor<T>): void;
  export function action<T>(callback: (...args: never[]) => T): () => T;
  export function merge<A, B>(first: A, second: B): A & B;
  export function omit<T extends object, K extends keyof T>(value: T, ...keys: K[]): Omit<T, K>;
}
