import "fz-proto-site-apply";
function Wrap() {
  const a = new Float32Array(1);
  a.reverse.apply(a, []);
  return null;
}
export function App() {
  return <Wrap />;
}
