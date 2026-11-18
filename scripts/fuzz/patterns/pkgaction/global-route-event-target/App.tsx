import "fz-global-route-event-target";
function Wrap() {
  void btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
