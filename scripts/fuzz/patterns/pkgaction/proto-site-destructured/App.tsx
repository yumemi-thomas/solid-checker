import "fz-proto-site-destructured";
function Wrap() {
  const { reverse } = new Float32Array(1);
  reverse();
  return null;
}
export function App() {
  return <Wrap />;
}
