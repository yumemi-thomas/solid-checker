import { createSignal } from "solid-js";
import { translator } from "@solid-primitives/i18n";

export default function App() {
  const [dict] = createSignal({ hello: "hello" }); const t = translator(() => dict());
  const current = t("hello");
  return <p>{String(current)}</p>;
}
