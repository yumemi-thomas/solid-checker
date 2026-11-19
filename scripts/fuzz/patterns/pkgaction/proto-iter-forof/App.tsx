import "fz-proto-iter-forof";
function Wrap() {
  for (const n of new Float32Array(1)) void n;
  return null;
}
export function App() {
  return <Wrap />;
}
