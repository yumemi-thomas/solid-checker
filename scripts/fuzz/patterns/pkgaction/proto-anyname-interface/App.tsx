import "fz-proto-anyname-interface";
interface Indexed {
  at(index: number): unknown;
}
function Wrap() {
  const x: Indexed = [1];
  void x.at(0);
  return null;
}
export function App() {
  return <Wrap />;
}
