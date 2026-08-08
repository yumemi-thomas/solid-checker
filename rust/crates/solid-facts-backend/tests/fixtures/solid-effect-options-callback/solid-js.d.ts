declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(
    compute: (previous?: T) => T,
    effect?: ((value: T) => void) | { effect?: (value: T) => void; error?: (error: unknown) => void },
  ): void;
}
