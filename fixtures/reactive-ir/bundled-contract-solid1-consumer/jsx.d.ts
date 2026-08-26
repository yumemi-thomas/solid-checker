declare namespace JSX {
  type Element = unknown;

  interface IntrinsicElements {
    button: { onClick?: unknown };
    div: Record<string, unknown>;
  }
}
