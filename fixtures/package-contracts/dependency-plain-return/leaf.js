// The dependency. Certified in the same published graph as each root, so a
// root's census reaches these exports through the leaf node's own verified
// export bindings.

// `returns` closes over one plain return: a conditional of literals is a
// primitive by its grammar alone (ADR 0113).
export function count(items) {
  return items ? 1 : 0;
}

// Completes without a value on every path, so `returns: []` closes (ADR 0035)
// and a call of it hands back `undefined`.
export function reset(items) {
  if (items) {
    return;
  }
}

// The 2026-09-28 amendment's hole: typed `number`, but a reassignable binding,
// so its plain return is refused and `returns` stays open. A root that
// forwards it has no closed claim to cite.
export function widened(key) {
  let value = 0;
  if (key === "unlock") value = () => 1;
  return value;
}
