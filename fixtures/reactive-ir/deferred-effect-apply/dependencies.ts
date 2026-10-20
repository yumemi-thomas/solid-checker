import { createSignal } from "solid-js";

const [dependency, setDependency] = createSignal(0);
export { setDependency };
export function readDependency() { return dependency(); }
export function emptyDependency() { return 0; }
