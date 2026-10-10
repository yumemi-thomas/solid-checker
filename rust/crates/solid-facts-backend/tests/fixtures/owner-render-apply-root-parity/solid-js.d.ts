declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
    div: {};
  }
  interface Element {}
}

// Every declaration the claims here rest on is copied verbatim from the
// published `@solidjs/signals@2.0.0-rc.3` `dist/types/signals.d.ts`
// (re-exported by `solid-js@2.0.0-rc.3` `types/client/hydration.d.ts` as
// `typeof coreRenderEffect` / `typeof coreEffect`), `core/owner.d.ts` and
// `core/types.d.ts`: the effect constructors with their `ComputeFunction`,
// `EffectFunction`, `EffectBundle` and `EffectOptions`, `onCleanup` with
// `Disposable`, `createRoot`, and `createMemo` with `MemoOptions`. JSDoc is
// dropped. `EffectOptions` and `MemoOptions` lose their `ssrSource` hydration
// augmentation, which no case passes, and `createMemo` its `loadingValue`
// overload, which no case uses. `createSignal` keeps only its plain-value
// overload and `SourceAccessor`'s `$REFRESH` brand is a local `unique
// symbol`; the signal only supplies reads and is no premise of any claim.
declare module "solid-js" {
  const $REFRESH: unique symbol;
  export type Refreshable<T> = T & {
    readonly [$REFRESH]: any;
  };
  export type Accessor<T> = () => T;
  export type SourceAccessor<T> = Refreshable<Accessor<T>>;
  export type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
  };
  export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
  export interface SignalOptions<T> {
    name?: string;
    equals?: false | ((prev: T, next: T) => boolean);
    ownedWrite?: boolean;
    unobserved?: () => void;
  }
  export function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;

  export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
  export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  export type EffectBundle<Prev, Next extends Prev = Prev> = {
    effect: EffectFunction<Prev, Next>;
    error: (err: unknown, cleanup: () => void) => void;
  };
  interface BaseEffectOptions {
    name?: string;
  }
  export interface EffectOptions extends BaseEffectOptions {
    defer?: boolean;
    schedule?: boolean;
    sync?: boolean;
    transparent?: boolean;
  }
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>): never;
  export function createRenderEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T>, options?: EffectOptions): void;

  export interface MemoOptions<T> {
    id?: string;
    name?: string;
    transparent?: boolean;
    equals?: false | ((prev: T, next: T) => boolean);
    unobserved?: () => void;
    lazy?: boolean;
    sync?: boolean;
    loadingValue?: T;
  }
  export function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;

  export interface Disposable {
    (): void;
  }
  export function onCleanup(fn: Disposable): Disposable;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: {
    id?: string;
    transparent?: boolean;
  }): T;
}
