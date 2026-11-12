import { action, flush } from "solid-js";
let Late: undefined | (new () => object);
const run = action(function* () {
  if (Late) new Late();
});
export function App() {
  setTimeout(() => {
    Late = class {
      x = flush();
    };
    void run();
  });
  return <p>x</p>;
}
