import { runStep } from "fz-step";
function Wrap() {
  [() => {}].forEach(runStep);
  return null;
}
export function App() {
  return <Wrap />;
}
