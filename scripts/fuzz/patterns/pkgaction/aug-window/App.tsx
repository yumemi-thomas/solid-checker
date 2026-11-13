import "fz-aug";
function Wrap() {
  window.fzRun(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
