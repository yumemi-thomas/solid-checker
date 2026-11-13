import kernel from "fz-obj";
const copy = Object.assign({}, kernel);
function Wrap() {
  copy.run(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
