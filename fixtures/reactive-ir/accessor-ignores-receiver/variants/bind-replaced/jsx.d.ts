declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: unknown) => void; children?: unknown; [attribute: string]: unknown };
    div: { children?: unknown; [attribute: string]: unknown };
  }
  interface Element {}
  interface ElementChildrenAttribute { children: {}; }
}
