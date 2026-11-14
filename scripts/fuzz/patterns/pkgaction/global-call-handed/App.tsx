import "fz-w";
const reader = (w: Window) => w.btoa("x");
function Wrap() {
  reader(window);
  return null;
}
export function App() {
  return <Wrap />;
}
