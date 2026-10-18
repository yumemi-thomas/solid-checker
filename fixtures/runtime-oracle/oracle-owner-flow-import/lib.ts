export function handlers(cb: () => void) {
  return { onClick: () => cb() };
}
