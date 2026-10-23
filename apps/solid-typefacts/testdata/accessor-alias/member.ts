import type { Accessor } from "solid-js";

export function install(props: { read: Accessor<number> }) {
  return props.read();
}
