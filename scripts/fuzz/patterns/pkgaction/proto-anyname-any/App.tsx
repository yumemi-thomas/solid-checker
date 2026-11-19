import "fz-proto-anyname-any";
function Wrap() {
  const x: any = [1];
  void x.at(0);
  return null;
}
export function App() {
  return <Wrap />;
}
