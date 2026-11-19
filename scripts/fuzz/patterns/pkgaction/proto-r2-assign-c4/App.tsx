import "fz-r2-s2";
const el = document.createElement("div");
const f = new Float32Array(1);
const d = new Date(0);
export function C0() {
  el.title = d as unknown as string;
  return null;
}
export function C1() {
  f[0] = d as unknown as number;
  return null;
}
export function C2() {
  el.style.width = d as unknown as string;
  return null;
}
export function C3() {
  el.textContent = d as unknown as string;
  return null;
}
export function C4() {
  location.hash = d as unknown as string;
  return null;
}
export function App() {
  return <C4 />;
}
