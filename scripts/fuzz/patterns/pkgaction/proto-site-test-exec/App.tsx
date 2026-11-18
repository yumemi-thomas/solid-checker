import "fz-proto-site-test-exec";
function Wrap() {
  void /fzprobe/.test("x");
  return null;
}
export function App() {
  return <Wrap />;
}
