import { makeVoid } from "fz-typed";
function mk() {
  return makeVoid();
}
const stored = mk();
function Wrap() {
  stored();
  return null;
}
export function App() {
  return <Wrap />;
}
