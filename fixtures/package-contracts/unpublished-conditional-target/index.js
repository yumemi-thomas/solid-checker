// The published default entrypoint. It certifies normally; the inapplicable
// sibling case must not suppress it.
export function observe(callback) {
  callback();
}
