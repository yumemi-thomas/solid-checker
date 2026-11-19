import "fz-proto-site-reflect-apply";
function Wrap() {
  const a = new Float32Array(1);
  Reflect.apply(a.reverse, a, []);
  return null;
}
export function App() {
  return <Wrap />;
}
