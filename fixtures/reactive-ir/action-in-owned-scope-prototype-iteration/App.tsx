import "fz-piter";


// ADR 1114 family 3 (ADR 1108 part 2): a collection constructor given a value runs the iterator `fz-piter` writes. A spread, `for…of` and an array pattern
// wait for slice C (held in `scripts/fuzz/open-defects.json`).
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
