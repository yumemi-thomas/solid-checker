import "fz-patch";
function Wrap() {
  new Float32Array(1).reverse();
  return null;
}
export function App() {
  return <Wrap />;
}
