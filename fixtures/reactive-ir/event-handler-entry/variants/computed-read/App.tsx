import { resolve } from "solid-js";

// Open: code holding the element reads a member under a key no constant names, which may be the stored handler's.
export function pick(target: object, key: string): unknown {
  return (target as Record<string, unknown>)[key];
}

export function Resolve() {
  return <button onClick={() => { resolve(() => 1); }}>go</button>;
}
