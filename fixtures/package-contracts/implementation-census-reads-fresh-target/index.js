// ADR 0153 item C: an accessor installation on a fresh target is bound to
// the exports that can operate on it.
//
// Both installations below allocate their target in the installing function
// and hand it back only as the return value, so the producer's census follows
// every use of it: `createView`'s proxy is operated on nowhere in the package,
// and `createBounds`' object only in `readWidth`. Every export but
// `readWidth` therefore proposes `reads` bounded against both sites, and the
// certifier's census confirms the bounds; `readWidth` can run a getter it
// installed, and its bound refuses.

// A proxy allocated fresh and returned. Nothing here reads a member of it.
export function createView(source) {
  return new Proxy({}, { get: (_, key) => source[key] });
}

// A fresh object that gains accessors inside a nested callback, then is
// returned. The callback's one use of it is the installation.
export function createBounds(keys) {
  const bounds = {};
  keys.forEach((key) => {
    Object.defineProperty(bounds, key, { get: () => 0, enumerable: true });
  });
  return bounds;
}

// Asks `createBounds`' result whether it has a key: an operation on that
// target (a `has` trap, were it a proxy), so a reader of that site. `in`
// leaves no form of its own, which is why the bound has to name it.
export function readWidth(keys) {
  const measured = createBounds(keys);
  return "width" in measured ? 1 : 0;
}

// Touches neither.
export function plainSum(a, b) {
  return a + b;
}
