import { action, flush } from "solid-js";
const run = action(function* (go: () => void) {
  go();
});
export function App() {
  setTimeout(() => {
    [1].forEach(function each(again?: unknown) {
      flush();
      if (again !== true) void run(() => each(true));
    });
  });
  return <p>x</p>;
}
