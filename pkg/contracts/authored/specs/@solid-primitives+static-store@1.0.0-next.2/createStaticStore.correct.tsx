import { createMemo } from "solid-js";
import { createStaticStore } from "@solid-primitives/static-store";
export default function App() {
  const [size] = createStaticStore({ count: 0 });
  const primed = createMemo(() => size.count);
  return <p>{String(size.count)} {String(primed())}</p>;
}
