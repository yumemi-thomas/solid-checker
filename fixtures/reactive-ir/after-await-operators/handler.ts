import { createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const loaded = (value: number) => Promise.resolve(value);

// ADR 1113: a promise handler runs after a suspension in a file with no `await` and no async function; its operands get their primitive domain all the same.
const coerced = { toString() { return String(source()); } };
const started = Date.now();

createRoot(() => {
  // A row: the coerced object.
  void loaded(1).then(() => `${coerced}`.length);
  // No row: two numbers.
  void loaded(2).then((value) => value - started);
});
