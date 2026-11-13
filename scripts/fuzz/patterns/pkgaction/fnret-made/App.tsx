import { makeHandle } from "fz-typed";
function mk() {
  return makeHandle();
}
const handle = mk();
function Wrap() {
  handle.dispose();
  return null;
}
export function App() {
  return <Wrap />;
}
