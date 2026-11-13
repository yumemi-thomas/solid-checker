import { makeVoid } from "fz-typed";
const stored = makeVoid();
function Wrap() {
  stored();
  return null;
}
export function App() {
  return <Wrap />;
}
