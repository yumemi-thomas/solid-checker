import "fz-patch";

// ADR 0999: the package replaced `btoa` through `document.documentElement.ownerDocument["defaultView"]`.
export function App() {
  const v = btoa("x");
  return <p>{v}</p>;
}
