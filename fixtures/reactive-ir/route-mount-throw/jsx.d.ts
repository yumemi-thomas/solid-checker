declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
    p: Record<string, unknown>;
    button: Record<string, unknown>;
  }
  interface Element {}
  interface ElementChildrenAttribute { children: {}; }
}
