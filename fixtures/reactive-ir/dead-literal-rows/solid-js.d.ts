declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    p: { children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}
