import "fz-patch";

// an escaped spelling of the member
export function App() {
  const v = window.\u0062toa("x");
  return <p>{v}</p>;
}
