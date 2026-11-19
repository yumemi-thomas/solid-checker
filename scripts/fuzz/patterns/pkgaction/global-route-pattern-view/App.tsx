import "fz-global-route-pattern-view";
function Wrap() {
  void btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
