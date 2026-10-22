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
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
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
}
