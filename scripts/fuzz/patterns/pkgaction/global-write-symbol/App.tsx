import "fz-wsymbol";
function Wrap() {
  btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
