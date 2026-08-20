declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
}

declare namespace JSX {
  interface IntrinsicElements {
    main: { children?: unknown };
    ul: { children?: unknown };
    ol: { children?: unknown };
    li: { children?: unknown };
    section: { children?: unknown };
    strong: { children?: unknown };
    span: { children?: unknown };
    div: { children?: unknown; innerHTML?: string; textContent?: string; onClick?: unknown; onclick?: unknown };
    button: { title?: string };
  }
  interface Element {}
}
