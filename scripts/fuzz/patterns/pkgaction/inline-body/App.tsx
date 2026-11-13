import { action } from "solid-js";
function Wrap() {
  void action(function* () {})();
  return null;
}
export function App() {
  return <Wrap />;
}
