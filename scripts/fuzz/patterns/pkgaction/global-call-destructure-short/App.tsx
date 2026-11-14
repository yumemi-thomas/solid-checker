import "fz-w";
function Wrap() {
  const { btoa } = globalThis;
  btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
