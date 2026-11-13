import { makeVoid } from "fz-typed";
const fns = [makeVoid()];
function Wrap() {
  fns[0]();
  return null;
}
export function App() {
  return <Wrap />;
}
