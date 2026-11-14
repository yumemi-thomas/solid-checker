import { makeVoid } from "fz-typed";
function id(f: VoidFunction) {
  return f;
}
const stored = id(makeVoid());
function Wrap() {
  stored();
  return null;
}
export function App() {
  return <Wrap />;
}
