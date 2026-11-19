import "fz-proto-site-call";
function Wrap() {
  const a = new Float32Array(1);
  a.reverse.call(a);
  return null;
}
export function App() {
  return <Wrap />;
}
