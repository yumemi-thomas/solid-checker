declare namespace JSX {
  interface IntrinsicElements {
    div: { id?: string; class?: string; title?: string; onClick?: (event: MouseEvent) => void; children?: unknown; [name: `data-${string}`]: string | undefined };
    span: { id?: string; class?: string; children?: unknown };
    input: { id?: string; value?: string; onChange?: (event: Event & { currentTarget: HTMLInputElement }) => void; ref?: (element: HTMLInputElement) => void };
    button: { type?: string; onClick?: (event: MouseEvent) => void; children?: unknown };
    p: { children?: unknown };
  }
  type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  type Element = RenderedElement | Node | ArrayElement | (string & {}) | number | boolean | null | undefined;
  interface ArrayElement extends Array<Element> {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Every declaration a claim here rests on is copied verbatim from the
// published 2.0.0-rc.9 typings, JSDoc dropped. From
// `@solidjs/signals/dist/types/signals.d.ts`: `Accessor`, `SourceAccessor`,
// `ComputeFunction`, `EffectFunction`, `EffectBundle`, `EffectOptions`,
// `MemoOptions`, `NoInfer`, `createMemo`, `createEffect`, `createRenderEffect`,
// `createTrackedEffect`, `onCleanup`, `onSettled`, with `Disposable` from
// `core/types.d.ts` and `Refreshable` from `core/constants.d.ts` (its
// `$REFRESH` brand is a local `unique symbol`). From
// `solid-js/types/types.d.ts`: `RenderedElement` and `ArrayElement`, composed
// into `JSX.Element` as `@solidjs/web/types/jsx.d.ts` does. From
// `solid-js/types/client/flow.d.ts`: `Show` (the two non-keyed overloads), `For`
// (the default-keyed overload) and `Loading`, where `SolidElement` is
// `JSX.Element`. `createSignal` keeps only its plain-value overload.
declare module "solid-js" {
  const $REFRESH: unique symbol;
  export type Refreshable<T> = T & {
    readonly [$REFRESH]: any;
  };
  export interface Disposable {
    (): void;
  }
  export type Accessor<T> = () => T;
  export type SourceAccessor<T> = Refreshable<Accessor<T>>;
  export type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
  };
  export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
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
  export interface SignalOptions<T> {
    name?: string;
    equals?: false | ((prev: T, next: T) => boolean);
    ownedWrite?: boolean;
    unobserved?: () => void;
  }
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
  export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
  export function onCleanup(fn: Disposable): Disposable;
  export function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
  export function createMemo<T>(compute: ComputeFunction<NoInfer<T>, T>, options: MemoOptions<T> & {
    loadingValue: T;
  }): SourceAccessor<T>;
  export function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
  export function createRenderEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T>, options?: EffectOptions): void;
  export function createTrackedEffect(compute: () => void | (() => void), options?: BaseEffectOptions): void;
  export function onSettled(callback: () => void | (() => void)): void;

  type SolidElement = JSX.Element;
  type NonZeroParams<T extends (...args: any[]) => any> = Parameters<T>["length"] extends 0 ? never : T;
  type ConditionalRenderCallback<T> = (item: Accessor<NonNullable<T>>) => SolidElement;
  export function Show<T>(props: {
    when: T | undefined | null | false;
    keyed?: false;
    fallback?: SolidElement;
    children: SolidElement;
  }): SolidElement;
  export function Show<T, F extends ConditionalRenderCallback<T>>(props: {
    when: T | undefined | null | false;
    keyed?: false;
    fallback?: SolidElement;
    children: NonZeroParams<F>;
  }): SolidElement;
  export function For<T extends readonly any[], U extends SolidElement>(props: {
    each: T | undefined | null | false;
    fallback?: SolidElement;
    keyed?: true;
    children: (item: T[number], index: Accessor<number>) => U;
  }): SolidElement;
  export function Loading(props: {
    fallback?: SolidElement;
    on?: any;
    children: SolidElement;
  }): SolidElement;
}

// `render` verbatim from `@solidjs/web/types/client.d.ts`, with its
// `MountableElement` alias and with the unused trailing options dropped.
declare module "@solidjs/web" {
  type MountableElement = Element | Document | ShadowRoot | DocumentFragment | Node;
  export function render(code: () => JSX.Element, element: MountableElement, init?: JSX.Element): () => void;
}
