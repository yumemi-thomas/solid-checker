declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
    main: { children?: any };
    button: { onClick?: unknown; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createMemo<T>(fn: () => T): () => T;
  export function Loading(props: { fallback?: JSX.Element; children?: JSX.Element | JSX.Element[] }): JSX.Element;
}

declare module "@solidjs/web" {
  export function httpStatus(code: number, text?: string): void;
  export function httpHeader(name: string, value: string, options?: { append?: boolean }): void;
  export function renderToStream(fn: () => JSX.Element): unknown;
}
