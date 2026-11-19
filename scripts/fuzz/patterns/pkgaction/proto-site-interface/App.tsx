import "fz-proto-site-interface";
interface Rev {
  reverse(): unknown;
}
function Wrap() {
  const a: Rev = new Float32Array(1);
  a.reverse();
  return null;
}
export function App() {
  return <Wrap />;
}
