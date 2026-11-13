import * as outer from "./re2";
function Wrap() {
  [() => {}].forEach(outer.ns.reRun);
  return null;
}
export function App() {
  return <Wrap />;
}
