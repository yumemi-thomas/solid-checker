import { makeVoid, } from "fz-typed";
const n = () => {};
const fns = [n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, n, makeVoid()];
function Wrap() {
  fns[16]();
  return null;
}
export function App() {
  return <Wrap />;
}
