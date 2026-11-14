import { makeVoid } from "fz-typed";
function mk(flag: boolean) {
  return flag ? () => {} : makeVoid();
}
const stored = mk(false);
function Wrap() {
  stored();
  return null;
}
export function App() {
  return <Wrap />;
}
