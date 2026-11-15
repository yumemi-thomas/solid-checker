import "fz-patch";

// ADR 0999: an identifier written with a Unicode escape is the global it spells; the package replaced `btoa` with a function that calls an action.
export function App() {
  const v = \u0062toa("x");
  return <p>{v}</p>;
}
