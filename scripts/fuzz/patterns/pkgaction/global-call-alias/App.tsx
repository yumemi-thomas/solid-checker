import "fz-w";
function Wrap() {
  const enc = window.btoa;
  enc("x");
  return null;
}
export function App() {
  return <Wrap />;
}
