import "fz-r2-t";
export function C0() {
  const e = new Error(); const re = /fzprobe/g;
  void e.message;
  return null;
}
export function C1() {
  const e = new Error(); const re = /fzprobe/g;
  void `${e}`;
  return null;
}
export function C2() {
  const e = new Error(); const re = /fzprobe/g;
  void ("" + e);
  return null;
}
export function C3() {
  const e = new Error(); const re = /fzprobe/g;
  void String(e);
  return null;
}
export function C4() {
  const e = new Error(); const re = /fzprobe/g;
  void e.toString();
  return null;
}
export function C5() {
  const e = new Error(); const re = /fzprobe/g;
  return <div>{e as unknown as string}</div>;
}
export function C6() {
  const e = new Error(); const re = /fzprobe/g;
  void `${re}`;
  return null;
}
export function C7() {
  const e = new Error(); const re = /fzprobe/g;
  void ("" + re);
  return null;
}
export function C8() {
  const e = new Error(); const re = /fzprobe/g;
  void re.flags;
  return null;
}
export function C9() {
  const e = new Error(); const re = /fzprobe/g;
  const { message } = e; void message;
  return null;
}
export function App() {
  return <C2 />;
}
