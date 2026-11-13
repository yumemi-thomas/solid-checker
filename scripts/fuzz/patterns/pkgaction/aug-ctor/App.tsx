import "fz-aug";
function Wrap() {
  Uint8Array.fzFrom(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
