import { createMemo } from "solid-js";
function tracked<T>(read: () => T) {
  return createMemo(() => read());
}
const wrappedLater: typeof tracked = ((read: () => unknown) => {
  const later = () => read();
  return later;
}) as typeof tracked;
export function App() {
  const a = createMemo(async () => 1);
  const options = wrappedLater(() => ({ enabled: a() }));
  return <main>{String(options().enabled)}</main>;
}
