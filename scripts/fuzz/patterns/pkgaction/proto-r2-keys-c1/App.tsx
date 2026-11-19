import "fz-r2-s2";
const rec: Record<string, number> = { a: 1 };
const d = new Date(0);
export function C0() {
  class K { [d as unknown as string]() {} }
  void K;
  return null;
}
export function C1() {
  const { [d as unknown as string]: x } = rec;
  void x;
  return null;
}
export function C2() {
  const a = [1, 2];
  a.length = d as unknown as number;
  return null;
}
export function C3() {
  document.title = d as unknown as string;
  return null;
}
export function C4() {
  let x = 0;
  ({ [d as unknown as string]: x } = rec);
  void x;
  return null;
}
export function App() {
  return <C1 />;
}
