import * as re from "./re";
const { reRun } = re;
function Wrap() {
  [() => {}].forEach(reRun);
  return null;
}
export function App() {
  return <Wrap />;
}
