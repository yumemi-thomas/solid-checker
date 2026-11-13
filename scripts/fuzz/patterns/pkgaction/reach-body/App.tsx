import { runStep } from "fz-step";
function Wrap() {
  void runStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
