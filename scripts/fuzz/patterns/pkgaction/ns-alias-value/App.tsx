import * as re from "./re";
const r = re;
function Wrap() {
  [() => {}].forEach(r.reRun);
  return null;
}
export function App() {
  return <Wrap />;
}
