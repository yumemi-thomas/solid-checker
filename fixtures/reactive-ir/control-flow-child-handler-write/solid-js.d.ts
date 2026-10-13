declare namespace JSX {
  interface IntrinsicElements {
    button: { type?: string; title?: string; onClick?: () => void; children?: unknown };
    div: { id?: string; onMouseEnter?: () => void; onClick?: () => void };
    section: {};
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Every declaration a claim here rests on is copied verbatim from the
// published 2.0.0-rc.9 typings, JSDoc dropped: `createSignal` with `Signal`,
// `SourceAccessor`, `Accessor`, `Setter` and `SignalOptions` from
// `@solidjs/signals/dist/types/signals.d.ts`, `Refreshable` from
// `core/constants.d.ts` (its `$REFRESH` brand is a local `unique symbol`), and
// the two non-keyed `Show` overloads with `NonZeroParams` and
// `ConditionalRenderCallback` from `solid-js/types/client/flow.d.ts`, where
// `SolidElement` is `JSX.Element`. `createSignal` keeps only its plain-value
// overload, and the keyed `Show` overloads are dropped: no case passes `keyed`.
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
}
