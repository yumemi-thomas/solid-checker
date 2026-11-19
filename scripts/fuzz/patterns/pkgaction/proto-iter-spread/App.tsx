import "fz-proto-iter-spread";
function Wrap() {
  void [...new Float32Array(1)];
  return null;
}
export function App() {
  return <Wrap />;
}
