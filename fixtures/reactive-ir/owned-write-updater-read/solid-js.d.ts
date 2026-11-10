declare namespace JSX {
  type Element = unknown;
  interface ElementChildrenAttribute { children: {}; }
  interface IntrinsicElements { div: { style?: unknown; children?: unknown }; p: { children?: unknown }; span: { children?: unknown }; b: { children?: unknown }; button: { onClick?: () => void; disabled?: boolean; children?: unknown }; }
}
declare module "@solidjs/web" { export namespace JSX { type Element = unknown; } }

// Claim-bearing signatures copied from published 2.0.0-rc.13:
// @solidjs/signals/dist/types/signals.d.ts, core/constants.d.ts and
// solid-js/types/client/core.d.ts, hydration.d.ts. Renderer/option scaffolding
// is reduced; effect returns retain the real cleanup-or-void constraint.
declare module "solid-js" {
  const $REFRESH: unique symbol;
  export type Refreshable<T> = T & { readonly [$REFRESH]: any };
  export type Accessor<T> = () => T;
  export type SourceAccessor<T> = Refreshable<Accessor<T>>;
  export type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
  };
  export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
  export interface SignalOptions<T> { name?: string; equals?: false | ((prev: T, next: T) => boolean); ownedWrite?: boolean; unobserved?: () => void; }
  export function createSignal<T>(): Signal<T | undefined>;
  export function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
  export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
  export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  export interface HydrationMemoOptions<T> { loadingValue?: T; }
  export const createMemo: {
    <T>(compute: ComputeFunction<NoInfer<T>, T>, options: HydrationMemoOptions<T> & { loadingValue: T }): SourceAccessor<T>;
    <T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: HydrationMemoOptions<T>): SourceAccessor<T>;
  };
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  export type EffectBundle<Prev, Next extends Prev = Prev> = { effect: EffectFunction<Prev, Next>; error: (err: unknown, cleanup: () => void) => void; };
  export interface EffectOptions { name?: string; }
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
  export type ContextProviderComponent<T> = (props: { value: T; children: JSX.Element }) => JSX.Element;
  export interface Context<T> extends ContextProviderComponent<T> { id: symbol; defaultValue: T | undefined; }
  export function createContext<T>(defaultValue?: T, options?: EffectOptions): Context<T>;
  export function useContext<T>(context: Context<T>): T;

  // Signatures verbatim from the published rc.17 declarations:
  // @solidjs/signals/dist/types/core/owner.d.ts, core/core.d.ts and map.d.ts.
  export interface Owner { readonly _root?: boolean; }
  export function getOwner(): Owner | null;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: { id?: string; transparent?: boolean; }): T;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  type Maybe<T> = T | void | null | undefined | false;
  export function mapArray<Item, MappedItem>(list: Accessor<Maybe<readonly Item[]>>, map: (value: Item, index: Accessor<number>) => MappedItem, options?: { keyed?: true; fallback?: Accessor<any>; name?: string; }): Accessor<MappedItem[]>;
  export function mapArray<Item, MappedItem>(list: Accessor<Maybe<readonly Item[]>>, map: (value: Accessor<Item>, index: number) => MappedItem, options: { keyed: false; fallback?: Accessor<any>; name?: string; }): Accessor<MappedItem[]>;
  export function mapArray<Item, MappedItem>(list: Accessor<Maybe<readonly Item[]>>, map: (value: Accessor<Item>, index: Accessor<number>) => MappedItem, options: { keyed: (item: Item) => any; fallback?: Accessor<any>; name?: string; }): Accessor<MappedItem[]>;
  export function repeat(count: Accessor<number>, map: (index: number) => any, options?: { from?: Accessor<number | undefined>; fallback?: Accessor<any>; name?: string; }): Accessor<any[]>;
}
