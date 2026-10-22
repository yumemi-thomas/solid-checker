declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: () => void; children?: unknown }; }
  interface Element extends Node {}
  interface ElementChildrenAttribute { children: {}; }
}
