// This fixture's JSX, reduced to the elements it renders.
declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void; children?: unknown };
    div: {};
  }
  interface Element {}
}
