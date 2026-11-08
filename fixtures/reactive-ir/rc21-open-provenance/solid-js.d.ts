declare namespace JSX {
  interface IntrinsicElements { div: {}; i: {} }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Component<P = {}> = (props: P) => any;
  export class NotReadyError extends Error {
    constructor(node: any);
  }
  export function createMemo<T>(compute: () => T | PromiseLike<T>): Accessor<T>;
  export function createSignal<T>(value: T | (() => T | PromiseLike<T>)): [Accessor<T>, (value: T) => void];
  export function lazy<T extends Component<any>>(load: () => Promise<{ default: T }>): T & { preload(): Promise<void> };
  export function Loading(props: { fallback?: any; children?: any }): any;
}
declare module "@solidjs/web" {
  export function render(code: () => any, element: any): () => void;
}
