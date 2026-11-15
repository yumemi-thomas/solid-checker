import "fz-late";

// the package loads the file that writes the global by a computed name
export function App() {
  const v = btoa("x");
  return <p>{v}</p>;
}
