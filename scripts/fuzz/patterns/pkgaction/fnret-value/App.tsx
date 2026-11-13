import { runStep } from "fz-step";
const pick = () => runStep;
function Wrap() {
  [() => {}].forEach(pick());
  return null;
}
export function App() {
  return <Wrap />;
}
