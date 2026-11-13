import { runStep } from "fz-dyn";
function Wrap() {
  void runStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
