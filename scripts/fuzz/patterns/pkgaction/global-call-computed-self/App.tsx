import "fz-w";
function Wrap() {
  globalThis["btoa"]("x");
  return null;
}
export function App() {
  return <Wrap />;
}
