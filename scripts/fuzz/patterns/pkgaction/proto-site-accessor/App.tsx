import "fz-proto-site-accessor";
function Wrap() {
  void new DataView(new ArrayBuffer(1)).byteLength;
  return null;
}
export function App() {
  return <Wrap />;
}
