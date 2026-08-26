declare module "solid-js" {
  export interface Owner {
    owned: unknown[] | null;
    cleanups: (() => void)[] | null;
    owner: Owner | null;
    context: unknown | null;
  }

  export var Owner: Owner | null;
  export type Component<P extends Record<string, any> = {}> = (props: P) => JSX.Element;
  export type Accessor<T> = () => T;
  export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Prev) => Next;
  export type Setter<T> = (value: T | ((prev: T) => T)) => T;
  export type Signal<T> = [get: Accessor<T>, set: Setter<T>];
  export interface SignalOptions<T> {
    name?: string;
    equals?: false | ((prev: T, next: T) => boolean);
    internal?: boolean;
  }

  export function createSignal<T>(): Signal<T | undefined>;
  export function createSignal<T>(value: T, options?: SignalOptions<T>): Signal<T>;
  export function createMemo<Next extends Prev, Prev = Next>(
    fn: EffectFunction<undefined | NoInfer<Prev>, Next>
  ): Accessor<Next>;
}
