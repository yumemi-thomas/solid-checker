import { makeVoid } from "fz-typed";
const box = { mk() { return makeVoid(); } };
const stored = box.mk();
function Wrap() {
  stored();
  return null;
}
export function App() {
  return <Wrap />;
}
