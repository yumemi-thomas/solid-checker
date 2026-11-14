import { runStep } from "fz-step";
function Wrap() {
  Array.from([runStep])[0](() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
