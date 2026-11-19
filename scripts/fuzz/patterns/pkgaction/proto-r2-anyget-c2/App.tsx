import "fz-r2-aa";
export function C0() {
  const dv = new DataView(new ArrayBuffer(1));
  void dv.byteLength;
  return null;
}
export function C1() {
  const dv = new DataView(new ArrayBuffer(1));
  const { byteLength } = dv; void byteLength;
  return null;
}
export function C2() {
  const dv = new DataView(new ArrayBuffer(1));
  void dv["byteLength"];
  return null;
}
export function C3() {
  const dv = new DataView(new ArrayBuffer(1));
  const k = "byteLength" as const; void dv[k];
  return null;
}
export function App() {
  return <C2 />;
}
