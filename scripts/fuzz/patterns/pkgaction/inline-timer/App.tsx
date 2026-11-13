import { action } from "solid-js";
function Wrap() {
  setTimeout(() => {
    void action(function* () {})();
  });
  return null;
}
export function App() {
  return <Wrap />;
}
