import "fz-global-route-contentwindow-parent";
function Wrap() {
  void btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
