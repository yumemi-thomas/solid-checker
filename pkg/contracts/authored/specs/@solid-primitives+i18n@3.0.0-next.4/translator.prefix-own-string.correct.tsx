import { createSignal } from "solid-js";
import { translator } from "@solid-primitives/i18n";

export default function App() {
  const [dict] = createSignal<Record<string, string>>({ hello: "hello" }); const t = translator(() => dict());
  return <p>{String(t(".hello"))}</p>;
}
