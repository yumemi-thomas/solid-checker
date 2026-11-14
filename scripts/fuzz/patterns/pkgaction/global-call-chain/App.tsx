import "fz-w";
function Wrap() {
  window.self.btoa("x");
  return null;
}
export function App() {
  return <Wrap />;
}
