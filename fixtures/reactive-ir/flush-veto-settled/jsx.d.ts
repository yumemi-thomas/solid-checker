// The intrinsic elements this fixture renders, reduced from `@solidjs/web`'s
// JSX types to the attributes it writes.
declare namespace JSX {
  interface IntrinsicElements {
    button: { children?: any; onClick?: (event: MouseEvent) => void };
    div: { children?: any };
  }
  interface Element {}
}
