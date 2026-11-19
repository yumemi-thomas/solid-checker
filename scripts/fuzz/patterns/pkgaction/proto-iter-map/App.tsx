import "fz-proto-iter-map";
function Wrap() {
  const pairs: [string, number][] & { fzprobe?: boolean } = [["k", 1]];
  pairs.fzprobe = true;
  void new Map(pairs);
  return null;
}
export function App() {
  return <Wrap />;
}
