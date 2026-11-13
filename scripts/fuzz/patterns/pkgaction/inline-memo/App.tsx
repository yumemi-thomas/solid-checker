import { action, createMemo } from "solid-js";
export function App() {
  const m = createMemo(() => {
    void action(function* () {})();
    return 1;
  });
  return <p>{m()}</p>;
}
