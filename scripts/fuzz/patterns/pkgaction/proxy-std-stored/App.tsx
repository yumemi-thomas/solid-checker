import { makeMap } from "fz-target";
const p = new Proxy(makeMap(), {});
function Wrap() {
  p.forEach(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
