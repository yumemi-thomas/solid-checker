// ADR 0488: onSettled after an await, with a callback that returns a cleanup, throws SETTLED_CLEANUP_UNOWNED (no owner on the continuation).
import { onSettled } from "solid-js";

export default function App() {
  const run = async () => {
    await Promise.resolve();
    onSettled(() => () => {});
  };
  void run();
  return <span id="w">ok</span>;
}
