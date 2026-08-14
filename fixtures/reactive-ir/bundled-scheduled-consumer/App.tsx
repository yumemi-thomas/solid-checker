import { debounce } from "@solid-primitives/scheduled";
import { createSignal } from "solid-js";

export function Search() {
  const [query] = createSignal("");
  const search = debounce(() => query(), 250);
  return <button onClick={search}>Search</button>;
}
