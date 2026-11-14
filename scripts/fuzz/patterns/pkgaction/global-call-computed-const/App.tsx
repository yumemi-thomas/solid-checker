import "fz-w";
function Wrap() {
  const k = "btoa";
  window[k]("x");
  return null;
}
export function App() {
  return <Wrap />;
}
