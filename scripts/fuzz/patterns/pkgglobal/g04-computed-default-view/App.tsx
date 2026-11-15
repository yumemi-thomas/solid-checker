import "fz-patch";

// the package writes through x["defaultView"]
export function App() {
  const v = btoa("x");
  return <p>{v}</p>;
}
