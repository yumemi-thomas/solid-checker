declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: unknown) => void; children?: unknown; [attribute: string]: unknown };
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

// Declarations copied from the published solid-js@2.0.0-rc.13 typings, JSDoc
// dropped: `createSignal` (plain-value overload), `Signal`, `Setter`,
// `SourceAccessor`, `Accessor`, `Refreshable` and `SignalOptions` from
// `@solidjs/signals/dist/types/signals.d.ts`; `action` from
// `@solidjs/signals/dist/types/core/action.d.ts:71`; `createRoot` from
// `core/owner.d.ts:133`.
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
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: { id?: string; transparent?: boolean }): T;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}

// The host `require` (what @types/node declares globally).
declare function require(id: string): unknown;

// Vite's `import.meta.glob` typing (vite/types/importGlob.d.ts, reduced).
interface ImportMeta {
  glob(pattern: string, options?: { eager?: boolean }): Record<string, unknown>;
}
