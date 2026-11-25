declare namespace JSX {
  interface IntrinsicElements { div: {}; p: { children?: unknown }; button: { onClick?: (event: MouseEvent) => void; children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
  export function createMemo<T>(compute: () => T): Accessor<T>;
  // `@solidjs/signals@2.0.0-rc.21`: `refresh<T>(target: Refreshable<T>): Promise<T>`; an accessor stands for the brand here.
  export type SourceAccessor<T> = Accessor<T>;
  export function refresh<T>(target: Accessor<T>): Promise<T>;
  // rc.21 reduced: the context object is its own provider component.
  export type Context<T> = ((props: { value: T; children?: unknown }) => JSX.Element) & { readonly id: symbol };
  export function createContext<T>(defaultValue?: T): Context<T>;
  export function useContext<T>(context: Context<T>): T;
}
