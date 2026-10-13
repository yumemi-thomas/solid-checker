import { createIsMounted } from "@solid-primitives/lifecycle";
export default function App() {
  const mounted = createIsMounted();
  const current = mounted(); // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
