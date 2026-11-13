import { makeMap } from "fz-target";
function Wrap() {
  new Proxy(makeMap(), {}).forEach(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
