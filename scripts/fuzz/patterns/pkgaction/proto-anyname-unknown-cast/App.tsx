import "fz-proto-anyname-unknown-cast";
function Wrap() {
  const x: unknown = [1];
  void (x as { at(index: number): unknown }).at(0);
  return null;
}
export function App() {
  return <Wrap />;
}
