declare module "solid-js" {
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createSignal<T>(value: T): [() => T, (next: T) => void];
  export function onSettled(fn: () => void): void;
  export function For<T>(props: { each: T[]; children: (item: T) => unknown }): unknown;
  export function Repeat(props: { count: number; children: (index: number) => unknown }): unknown;
  export function Show<T>(props: { when: T; keyed?: boolean; children: unknown }): unknown;
  export function Switch(props: { children: unknown }): unknown;
  export function Match<T>(props: { when: T; children: unknown }): unknown;
}
