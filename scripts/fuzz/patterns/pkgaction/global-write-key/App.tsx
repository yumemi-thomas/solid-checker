import "fz-wkey";
function Wrap() {
  btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
