// This fixture's JSX, reduced to the elements it renders.
declare namespace JSX {
  interface IntrinsicElements {
    div: {};
    button: { onClick?: () => void };
  }
  interface Element {}
}
