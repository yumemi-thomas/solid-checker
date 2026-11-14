import "fz-pub";
function Wrap() {
  window.__fzAct(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
