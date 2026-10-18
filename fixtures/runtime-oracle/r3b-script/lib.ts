// A script, not a module: these declarations are globals any file can call.
function relay(f: () => void) {
  return { onClick: () => f(), now: () => f() };
}
function fireNow(h: { now: () => void }) {
  h.now();
}
