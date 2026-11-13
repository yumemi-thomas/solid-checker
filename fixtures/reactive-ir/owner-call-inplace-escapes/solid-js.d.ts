declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  // Reduced from `solid-js@2.0.0-rc.21` `types/types.d.ts` (`Element`, `ArrayElement`): no function is an element, so `Show`'s function-child overloads are reached.
  interface ArrayElement extends Array<Element> {}
  type Element = Node | ArrayElement | (string & {}) | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}
