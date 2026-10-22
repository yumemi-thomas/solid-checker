import { createSignal } from "solid-js";
import { useLabel } from "dep-legacy";
import { useOkLabel } from "dep-ok";

// The package whose closure the compiler refuses stays off source.
export function Legacy() {
  const [name] = createSignal("a");
  const label = useLabel(name);
  return <p>{label}</p>;
}

// The package the compiler accepts is analyzed from source.
export function Accepted() {
  const [name] = createSignal("a");
  const label = useOkLabel(name);
  return <p>{label}</p>;
}
