// `Component` and its element types are transcribed from solid-js@2.0.0-rc.17
// (`types/client/component.d.ts:6`, `types/types.d.ts:8-15`) and `Accessor`
// from @solidjs/signals@2.0.0-rc.17 (`dist/types/signals.d.ts:55`). Doc
// comments are dropped and `export declare` is spelled `export` inside the
// ambient block. `createSignal` and `For` are reduced stubs.

declare namespace JSX {
  interface IntrinsicElements { b: {}; i: {}; p: {}; div: { children?: unknown } }
  type Element = SolidElement;
  type SolidElement = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  interface ArrayElement extends Array<SolidElement> {}
}

declare module "solid-js" {
  type SolidElement = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  interface ArrayElement extends Array<SolidElement> {
  }
  export type Accessor<T> = () => T;
  export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function For<T>(props: { each: readonly T[]; children: (item: T, index: () => number) => unknown }): unknown;
}
