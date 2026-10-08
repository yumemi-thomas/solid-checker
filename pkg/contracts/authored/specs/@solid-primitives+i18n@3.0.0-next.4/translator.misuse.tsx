import { createMemo, createSignal } from "solid-js";
import { translator } from "@solid-primitives/i18n";
export default function App() {
  const [dictionary] = createSignal({ hello: "hello" });
  const translate = translator(dictionary);
  const current = translate("hello");
  void current;
  return document.createElement("p");
}
