import type { Accessor } from "solid-js";

export function install(read: Accessor<number>) {
  return read();
}
