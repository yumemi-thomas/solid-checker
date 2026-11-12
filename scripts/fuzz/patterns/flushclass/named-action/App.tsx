import { action, flush } from "solid-js";
export function App() {
  setTimeout(function tick(again?: boolean) {
    flush();
    if (!again) {
      const run = action(function* () {
        tick(true);
      });
      void run();
    }
  });
  return <p>x</p>;
}
