import "fz-wwin";
function Wrap() {
  window.btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
