import "fz-patch";

// an escaped spelling of the global
export function App() {
  const v = \u0062toa("x");
  return <p>{v}</p>;
}
