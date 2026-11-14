import "fz-ctor";
function Wrap() {
  new Float32Array("fz", () => {});
  return null;
}
export function App() {
  return <Wrap />;
}
