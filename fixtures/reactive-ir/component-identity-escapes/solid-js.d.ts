// `Show`, `Component`, `ComponentProps`, `ValidComponent` and their helper
// types are transcribed from solid-js@2.0.0-rc.17 (`types/client/flow.d.ts:4-9`
// and `:89-112`, `types/client/component.d.ts:6`, `:47` and `:54`,
// `types/types.d.ts:8-15`) and `Accessor` from @solidjs/signals@2.0.0-rc.17
// (`dist/types/signals.d.ts:55`). Doc comments are dropped and `export declare`
// is spelled `export` inside the ambient block. The other declarations are
// reduced stubs.

declare namespace JSX {
  interface IntrinsicElements { li: {}; ul: {}; b: {}; p: {} }
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
  type NonZeroParams<T extends (...args: any[]) => any> = Parameters<T>["length"] extends 0 ? never : T;
  type ConditionalRenderCallback<T> = (item: Accessor<NonNullable<T>>) => SolidElement;
  type KeyedConditionalRenderCallback<T> = (item: NonNullable<T>) => SolidElement;
  export type Accessor<T> = () => T;
  export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;
  export type ValidComponent = Component<any>;
  export type ComponentProps<T extends ValidComponent> = T extends Component<infer P> ? P : never;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
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
  export function untrack<T>(fn: () => T): T;
  export function For<T>(props: { each: readonly T[]; children: (item: T, index: () => number) => unknown }): unknown;
}
