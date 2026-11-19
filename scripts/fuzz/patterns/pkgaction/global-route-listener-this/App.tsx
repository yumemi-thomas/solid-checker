import "fz-global-route-listener-this";
function Wrap() {
  void btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
