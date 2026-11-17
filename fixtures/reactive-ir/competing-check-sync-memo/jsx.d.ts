// This fixture's JSX, reduced to the elements it renders.
declare namespace JSX {
  interface IntrinsicElements {
    i: {};
    button: { onClick?: () => void };
  }
  interface Element {}
}
