import { createScheduled } from "@solid-primitives/scheduled";
export default function App() {
  const scheduled = createScheduled(invalidate => {
    invalidate();
    return () => {};
  });
  const current = scheduled();
  void current;
  return document.createElement("p");
}
