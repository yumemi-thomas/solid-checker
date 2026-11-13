import kernel from "fz-obj";
function Wrap() {
  [() => {}].forEach(kernel.run);
  return null;
}
export function App() {
  return <Wrap />;
}
