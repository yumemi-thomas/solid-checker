declare namespace JSX {
  interface IntrinsicElements { div: Record<string, unknown>; }
  interface Element {}
  interface ElementChildrenAttribute { children: {}; }
  // A Solid component may return `undefined`; the compiler, not React's
  // contract, renders it.
  type ElementType = string | ((props: any) => unknown);
}
