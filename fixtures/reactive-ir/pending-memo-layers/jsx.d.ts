declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: () => void; children?: unknown }; div: { title?: string; children?: unknown }; p: { children?: unknown }; }
  interface Element extends Node {}
  interface ElementChildrenAttribute { children: {}; }
}
