// Only a dynamic import: still a script for TypeScript (and ECMAScript).
void import("./other");
function relay(f: () => void) {
  return { onClick: () => f(), now: () => f() };
}
function fireNow(g: () => void) {
  g();
}
