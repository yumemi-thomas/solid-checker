const el = document.body;
Object.keys(window);
const keys = Object.keys;
keys(window);
window.scrollTo(0, 0);
window.setTimeout(() => {}, 0);
const hasTimers = typeof setTimeout === "function";
document.body.addEventListener("x", (e) => {
  const path = e.composedPath();
  if (hasTimers && path.includes(el) && path.indexOf(el) >= 0 && path.length > 0) return;
  e.composedPath().some((t) => t === el);
});
export {};
