import { createErrorBoundary } from "solid-js/internal";
export function App() {
  const m = createErrorBoundary(() => 1, () => 2);
  return <p>{m()}</p>;
}
