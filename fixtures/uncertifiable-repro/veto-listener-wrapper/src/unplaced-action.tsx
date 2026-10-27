// inferay InstructionsEditor.tsx:32-46: an action called from a click handler,
// in a project whose one synchronous dispatch (keyboard.tsx) withholds every
// handler's proofs (ADR 0356), so where the action starts is not placed.
import { action, createSignal } from "solid-js";

export function InstructionsEditor() {
  const [saved, setSaved] = createSignal("");
  const save = action(function* (value: string) {
    yield Promise.resolve(value);
    setSaved(value);
  });
  return <button onClick={() => void save("text")}>{saved()}</button>;
}
