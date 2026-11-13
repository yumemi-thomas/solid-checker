import { reRun } from "./re";
function Wrap() {
  [() => {}].forEach(reRun);
  return null;
}
export function App() {
  return <Wrap />;
}
