import "fz-wevent";
function Wrap() {
  btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
