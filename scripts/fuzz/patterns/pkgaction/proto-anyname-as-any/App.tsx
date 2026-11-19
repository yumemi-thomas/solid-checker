import "fz-proto-anyname-as-any";
function Wrap() {
  void ([1] as any).at(0);
  return null;
}
export function App() {
  return <Wrap />;
}
