import { makeVoid } from "fz-typed";
class Box { f = makeVoid(); }
const box = new Box();
function Wrap() {
  box.f();
  return null;
}
export function App() {
  return <Wrap />;
}
