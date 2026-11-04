// The intrinsic elements this fixture renders, reduced from `@solidjs/web`'s JSX types to the attributes it writes.
declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
    p: { children?: any };
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

