// Declarations transcribed from solid-js@2.0.0-rc.9 and @solidjs/web@2.0.0-rc.9.
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - `IntrinsicElement`, `ValidComponent`, `ComponentProps`
//                  @solidjs/web `types/index.d.ts:46-48`
// - `DynamicOptions` and `dynamic`
//                  @solidjs/web `types/index.d.ts:82-97` (doc comments kept).
//                  The source is typed `() => T | Promise<T> | null |
//                  undefined | false` whatever `static` says, which is why
//                  `tsc` accepts every promise-valued static source here.
// - `Component`    solid-js `types/client/component.d.ts:6`
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files are modules, so they spell `export declare
// function`, and inside the block that is `export function`.)
//
// Reduced, and safe to reduce because no claim reads them back: `SolidElement`
// is this file's `JSX.Element` rather than solid-js' `Element` union, and
// `JSX` itself. `Promise` is the default library's, as in any project.

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
      /**
       * The source cannot change: call it once, untracked, now, and render the
       * result with no computation per instance (see `dynamic`). The source must
       * resolve synchronously.
       */
      static?: boolean;
  }
  export function dynamic<T extends ValidComponent>(source: () => T | Promise<T> | null | undefined | false, options?: DynamicOptions): Component<ComponentProps<T>>;
}
