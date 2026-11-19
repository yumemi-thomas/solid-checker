import "fz-piter";


// ADR 1108 part 2: each iteration runs the iterator `fz-piter` writes.
export function Spread() {
  void [...new Float32Array(1)];
  return null;
}
export function ForOf() {
  for (const n of new Float32Array(1)) void n;
  return null;
}
export function ArrayPattern() {
  const [n] = new Float32Array(1);
  void n;
  return null;
}
export function SetConstructor() {
  void new Set(new Float32Array(1));
  return null;
}
export function MapConstructor() {
  const pairs: [string, number][] & { fzprobe?: boolean } = [["k", 1]];
  pairs.fzprobe = true;
  void new Map(pairs);
  return null;
}

// Unchanged (ADR 1108 part 1): a call of a library method, which the any-name writer may have written.
export function LibraryCall() {
  void new Float32Array(1).slice();
  return null;
}

// Silent: the iteration in a handler.
export function SpreadInHandler() {
  return <button onClick={() => void [...new Float32Array(1)]}>spread</button>;
}
