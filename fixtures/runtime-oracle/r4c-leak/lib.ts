export function handlers(cb: () => void) {
  return { onClick: () => cb() };
}
export function inMemo(cb: () => void) {
  return () => cb();
}
