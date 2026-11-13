import { runStep } from "fz-src";
function Wrap() {
  void runStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
