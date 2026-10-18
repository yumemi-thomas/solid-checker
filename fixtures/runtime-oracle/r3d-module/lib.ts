export function relay(f: () => void) {
  return { onClick: () => f(), now: () => f() };
}
export function fireNow(g: () => void) {
  g();
}
