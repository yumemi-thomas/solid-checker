import { createSignal } from "solid-js";

export const App = () => {
  const [ready] = createSignal(true);
  return <main>{ready() && <span>ready</span>}</main>;
};
