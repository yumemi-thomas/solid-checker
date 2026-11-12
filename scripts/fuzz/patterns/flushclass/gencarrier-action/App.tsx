import { action, flush } from "solid-js";
let late: undefined | Generator<void, void, unknown>[];
const run = action(function* () {
  late?.[0]?.next();
});
export function App() {
  setTimeout(() => {
    late = [1].map(function* () {
      flush();
    });
    void run();
  });
  return <p>x</p>;
}
