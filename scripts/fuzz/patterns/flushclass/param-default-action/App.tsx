import { action, flush } from "solid-js";
let late: undefined | ((x?: void) => void);
const run = action(function* () {
  late?.();
});
export function App() {
  setTimeout(() => {
    late = (x = flush()) => x;
    void run();
  });
  return <p>x</p>;
}
