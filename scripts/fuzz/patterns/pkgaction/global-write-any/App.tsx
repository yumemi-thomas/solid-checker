import "fz-wany";
function Wrap() {
  btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
