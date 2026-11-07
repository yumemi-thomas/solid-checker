import { createEffect } from "solid-js";
export function App() {
  const u1 = () => {
    createEffect(() => 1, () => {}, { sync: true });
  };
  createEffect(() => {
    return 1;
  }, (v) => {
    u1();
  });
  return <i>app</i>;
}
