declare namespace JSX {
  type Element = import("solid-js").Element;
  interface IntrinsicElements {
    a: Record<string, unknown>;
    span: Record<string, unknown>;
  }
  interface ElementChildrenAttribute {
    children: {};
  }
}
