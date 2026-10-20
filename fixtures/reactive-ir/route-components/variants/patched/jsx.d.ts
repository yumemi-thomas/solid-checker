declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
    article: Record<string, unknown>;
    h2: Record<string, unknown>;
    p: Record<string, unknown>;
    input: Record<string, unknown>;
  }
  interface Element {}
  interface ElementChildrenAttribute { children: {}; }
}
