export function imported(cb: () => void) {
  return { onClick: () => cb() };
}
