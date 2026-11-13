import { runStep } from "fz-pure";
function Wrap() {
  void runStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
