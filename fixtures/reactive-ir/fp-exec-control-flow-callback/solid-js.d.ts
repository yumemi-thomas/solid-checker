declare namespace JSX {
  interface IntrinsicElements {
    div: { title?: string; children?: unknown };
    b: { children?: unknown };
    span: { children?: unknown };
    li: { children?: unknown };
    ul: { children?: unknown };
  }
  type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  type Element = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  interface ArrayElement extends Array<Element> {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Every declaration a claim here rests on is copied from the published
// 2.0.0-rc.9 typings, JSDoc dropped: `createSignal` with its helper types from
// `@solidjs/signals/dist/types/signals.d.ts` (plain-value overload only),
// `createMemo` (the overload without a required loadingValue), and
// `For`/`Repeat`/`Show`/`Switch`/`Match` plus their helper types verbatim from
// `solid-js/types/client/flow.d.ts`. Only `Element` is local: the real one is
// `RenderedElement | ArrayElement | ...` from `solid-js/types/types.d.ts`, which
// the global `JSX.Element` below spells out so that JSX expressions type-check
// without the `@solidjs/web` JSX runtime declarations.
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

  export type Element = JSX.Element;
  type SolidElement = Element;
  type NonZeroParams<T extends (...args: any[]) => any> = Parameters<T>["length"] extends 0 ? never : T;
  type ConditionalRenderCallback<T> = (item: Accessor<NonNullable<T>>) => SolidElement;
  type KeyedConditionalRenderCallback<T> = (item: NonNullable<T>) => SolidElement;
  type ConditionalRenderChildren<T, F extends ConditionalRenderCallback<T> = ConditionalRenderCallback<T>> = SolidElement | NonZeroParams<F>;
  type KeyedConditionalRenderChildren<T, F extends KeyedConditionalRenderCallback<T> = KeyedConditionalRenderCallback<T>> = SolidElement | NonZeroParams<F>;
  export function For<T extends readonly any[], U extends SolidElement>(props: {
    each: T | undefined | null | false;
    fallback?: SolidElement;
    keyed?: true;
    children: (item: T[number], index: Accessor<number>) => U;
  }): SolidElement;
  export function For<T extends readonly any[], U extends SolidElement>(props: {
    each: T | undefined | null | false;
    fallback?: SolidElement;
    keyed: false;
    children: (item: Accessor<T[number]>, index: number) => U;
  }): SolidElement;
  export function For<T extends readonly any[], U extends SolidElement>(props: {
    each: T | undefined | null | false;
    fallback?: SolidElement;
    keyed: (item: T[number]) => any;
    children: (item: Accessor<T[number]>, index: Accessor<number>) => U;
  }): SolidElement;
  export function Repeat<T extends SolidElement>(props: {
    count: number;
    from?: number | undefined;
    fallback?: SolidElement;
    children: ((index: number) => T) | T;
  }): SolidElement;
  export function Show<T>(props: {
    when: T | undefined | null | false;
    keyed: true;
    fallback?: SolidElement;
    children: SolidElement;
  }): SolidElement;
  export function Show<T, F extends KeyedConditionalRenderCallback<T>>(props: {
    when: T | undefined | null | false;
    keyed: true;
    fallback?: SolidElement;
    children: NonZeroParams<F>;
  }): SolidElement;
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
  export function Switch(props: {
    fallback?: SolidElement;
    children: SolidElement;
  }): SolidElement;
  export type MatchProps<T, F extends ConditionalRenderCallback<T> = ConditionalRenderCallback<T>> = {
    when: T | undefined | null | false;
    keyed?: false;
    children: ConditionalRenderChildren<T, F>;
  };
  export type KeyedMatchProps<T, F extends KeyedConditionalRenderCallback<T> = KeyedConditionalRenderCallback<T>> = {
    when: T | undefined | null | false;
    keyed: true;
    children: KeyedConditionalRenderChildren<T, F>;
  };
  export type AnyMatchProps<T> = MatchProps<T> | KeyedMatchProps<T> | {
    when: T | undefined | null | false;
    keyed?: boolean;
    children: SolidElement;
  };
  export function Match<T>(props: {
    when: T | undefined | null | false;
    keyed: true;
    children: SolidElement;
  }): SolidElement;
  export function Match<T, F extends KeyedConditionalRenderCallback<T>>(props: KeyedMatchProps<T, F>): SolidElement;
  export function Match<T>(props: {
    when: T | undefined | null | false;
    keyed?: false;
    children: SolidElement;
  }): SolidElement;
  export function Match<T, F extends ConditionalRenderCallback<T>>(props: MatchProps<T, F>): SolidElement;
  export function Match<T>(props: AnyMatchProps<T>): SolidElement;
}
