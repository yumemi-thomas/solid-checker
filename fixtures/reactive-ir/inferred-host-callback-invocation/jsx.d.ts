declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
    button: { onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
  interface ElementChildrenAttribute { children: {}; }
}
