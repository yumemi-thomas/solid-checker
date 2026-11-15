import "fz-patch";

// control: the name as it is
export function App() {
  const v = btoa("x");
  return <p>{v}</p>;
}
