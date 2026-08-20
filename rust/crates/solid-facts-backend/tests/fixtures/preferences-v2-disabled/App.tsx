import { createSignal } from "solid-js";

export const App = () => {
  const [items] = createSignal(["a", "b"]);
  const [ready] = createSignal(true);
  return (
    <main>
      {items().map((item) => <span>{item}</span>)}
      {ready() && <span>ready</span>}
    </main>
  );
};
