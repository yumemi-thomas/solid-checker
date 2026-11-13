import kernel from "fz-obj";
function Wrap() {
  Object.freeze(kernel).run(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
