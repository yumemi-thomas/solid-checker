import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Uncertifiable: an anonymous default export has no binding whose references
// reach its importers' uses (`import Row from "./Default"; items().map(Row)`).
export default () => {
  const value = count();
  return <b>{value}</b>;
};
