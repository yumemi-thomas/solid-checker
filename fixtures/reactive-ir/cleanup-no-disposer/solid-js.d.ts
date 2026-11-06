// Declarations transcribed from solid-js@2.0.0-rc.17, @solidjs/signals@2.0.0-rc.17
// and @solidjs/web@2.0.0-rc.17.
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - `DynamicOptions` and `dynamicComponent`
//                  solid-js `types/client/dynamic.d.ts:6-21`, `:53-55` (doc
//                  comments reduced); the source type excludes strings
// - `Component`, `ComponentProps`
//                  solid-js `types/client/component.d.ts:6`, `:54`
// - `untrack`      @solidjs/signals `dist/types/core/core.d.ts:167`
// - `@solidjs/web` re-exports `dynamicComponent` from `solid-js`
//                  (`types/index.d.ts`, `dist/web.dev.js:2`)
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files are modules, so they spell `export declare
// function`, and inside the block that is `export function`.)
//
// Reduced, and safe to reduce because no claim reads them back: `SolidElement`
// is this file's `JSX.Element`; `createSignal` keeps only its value overload
// and `createEffect` its two-function shape (rc.17 `dist/types/signals.d.ts:302`
// and `:427`), without the `ComputeFunction`/`EffectFunction` inference
// machinery, which is no looser for any argument written here; and `JSX`
// itself.

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
    span: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  type SolidElement = JSX.Element;
  export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;
  export type ValidComponent = Component<any>;
  export type ComponentProps<T extends ValidComponent> = T extends Component<infer P> ? P : never;
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  // Published rc.17 dist/types/core/scheduler.d.ts:384-385.
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
  export function onCleanup(fn: () => void): void;
  export function onSettled(fn: () => void | (() => void)): void;
  // ADR 0571: published rc.17 `@solidjs/signals` dist/types/store/types.d.ts
  // (`Store` :7, `StoreSetter` :24, `StoreReturn` :26, `NoFn` :71, `StoreOptions`
  // reduced to `name`) and store/index.d.ts:33 (`createStore`'s plain overload).
  export type Store<T> = T;
  export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
  export type StoreReturn<T> = [get: Store<T>, set: StoreSetter<T>];
  export type NoFn<T> = T extends Function ? never : T;
  export interface StoreOptions { name?: string; }
  export function createStore<T extends object = {}>(initialValue: NoFn<T>, options?: StoreOptions): StoreReturn<T>;
  export interface DynamicOptions {
      /**
       * SSR only: hold the document's first flush until the source settles.
       */
      deferStream?: boolean;
      /**
       * The source cannot change: call it once, untracked, now, and render the
       * result with no computation per instance. The source must resolve
       * synchronously.
       */
      static?: boolean;
  }
  export const dynamicComponent: {
      <C extends Component<any>>(source: () => C | Promise<C> | AsyncIterable<C> | null | undefined | false, options?: DynamicOptions): Component<ComponentProps<C>>;
  };
}

declare module "@solidjs/web" {
  export { dynamicComponent } from "solid-js";
  export function render(code: () => unknown, element: Element): () => void;
}
