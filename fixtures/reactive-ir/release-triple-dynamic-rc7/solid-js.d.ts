// Declarations transcribed from solid-js@2.0.0-rc.7 and @solidjs/web@2.0.0-rc.7.
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - `IntrinsicElement`, `ValidComponent`, `ComponentProps`
//                  @solidjs/web `types/index.d.ts:46-48`
// - `DynamicOptions` and `dynamic`
//                  @solidjs/web `types/index.d.ts:81-90` (doc comment kept):
//                  `deferStream` only, so `{ static: true }` is TS2353
// - `Component`    solid-js `types/client/component.d.ts:6`
// - `untrack`      @solidjs/signals `dist/types/core/core.d.ts:95`
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files are modules, so they spell `export declare
// function`, and inside the block that is `export function`.)
//
// Reduced, and safe to reduce because no claim reads them back: `SolidElement`
// is this file's `JSX.Element` rather than solid-js' `Element` union;
// `createSignal` keeps only its value overload and `createEffect` its
// two-function shape (rc.7 `dist/types/signals.d.ts:263` and `:367`), without
// the `ComputeFunction`/`EffectFunction`/`Setter` inference machinery, which is
// no looser for any argument written here; and `JSX` itself.

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  type SolidElement = JSX.Element;
  export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}

declare module "@solidjs/web" {
  import { Component } from "solid-js";
  export type IntrinsicElement = Extract<keyof JSX.IntrinsicElements, string>;
  export type ValidComponent = IntrinsicElement | Component<any> | (string & {});
  export type ComponentProps<T extends ValidComponent> = T extends Component<infer P> ? P : T extends keyof JSX.IntrinsicElements ? JSX.IntrinsicElements[T] : Record<string, unknown>;
  export interface DynamicOptions {
      /**
       * SSR only: hold the document's first flush until the source settles, so
       * the resolved component renders into the shell instead of streaming in
       * behind its boundary's fallback. Same meaning as `createMemo`'s
       * `deferStream`. Ignored on the client.
       */
      deferStream?: boolean;
  }
  export function dynamic<T extends ValidComponent>(source: () => T | Promise<T> | null | undefined | false, _options?: DynamicOptions): Component<ComponentProps<T>>;
}
