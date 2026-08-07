declare namespace JSX {
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
  interface IntrinsicElements {
    a: Record<string, unknown>;
    button: Record<string, unknown>;
    div: Record<string, unknown>;
  }
}
