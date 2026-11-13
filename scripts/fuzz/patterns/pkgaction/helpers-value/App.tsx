import { helpers } from "fz-help";
const run = (f: (cb: () => void) => void) => f(() => {});
function Wrap() {
  run(helpers.runStep);
  return null;
}
export function App() {
  return <Wrap />;
}
