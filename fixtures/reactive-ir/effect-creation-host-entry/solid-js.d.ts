declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    button: { onClick?: () => void; "data-act"?: string; children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}
