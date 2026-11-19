import "fz-proto-site-bind";
function Wrap() {
  const a = new Float32Array(1);
  a.reverse.bind(a)();
  return null;
}
export function App() {
  return <Wrap />;
}
