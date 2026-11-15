import { makeHandle } from "fz-typed";
const handle = makeHandle();
function Wrap() {
  handle.dispose();
  return null;
}
export function App() {
  return <Wrap />;
}
