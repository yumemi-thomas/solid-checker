import "fz-proto-site-json-tojson";
function Wrap() {
  void JSON.stringify({ at: new Date(0) });
  return null;
}
export function App() {
  return <Wrap />;
}
