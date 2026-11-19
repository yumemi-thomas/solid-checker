import "fz-proto-iter-set";
function Wrap() {
  void new Set(new Float32Array(1));
  return null;
}
export function App() {
  return <Wrap />;
}
