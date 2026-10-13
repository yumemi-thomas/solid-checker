import { createSignal } from "solid-js";
import { otherReadSignal } from "./other.js";

export function readSignal() {
  const [here] = createSignal("here");
  return here();
}

export function composesTheLocalTarget() {
  return readSignal();
}

export function composesTheImportedTarget() {
  return otherReadSignal();
}
