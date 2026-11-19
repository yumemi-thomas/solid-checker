import "fz-r2-ac";
const dv = new DataView(new ArrayBuffer(1));
const w = dv as unknown as { fzprobe?: number };
export function C0() {
  void dv.getInt8(0);
  return null;
}
export function C1() {
  void ("fzprobe" in dv);
  return null;
}
export function C2() {
  void w.fzprobe;
  return null;
}
export function C3() {
  const { fzprobe } = w;
  void fzprobe;
  return null;
}
export function App() {
  return <C2 />;
}
