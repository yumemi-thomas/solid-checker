// The JSX namespace for the reduced `solid-js` and `@solidjs/web` stubs in node_modules.
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    i: {};
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute {
    children: {};
  }
}
