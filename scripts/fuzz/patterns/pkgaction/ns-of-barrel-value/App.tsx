import * as re from "./re";
function Wrap() {
  [() => {}].forEach(re.reRun);
  return null;
}
export function App() {
  return <Wrap />;
}
