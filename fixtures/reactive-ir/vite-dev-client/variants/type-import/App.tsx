// Each site runs a member of a built-in, or a global, that the package named `vite` may write (its Node code writes under keys nothing decides).
export function CalledDirectly() {
  new Float32Array(1).reverse();
  return null;
}
export function CalledOnAny() {
  const a: any = new Float32Array(1);
  a.reverse();
  return null;
}
export function TemplateSubstitution() {
  void `${["fzprobe"]}`;
  return null;
}
export function Mapped() {
  void [1, 2].map((n) => n * 2);
  return null;
}
// Vite 8's dev client replaces `console.error` and `console.warn` (console forwarding): the site meets the package as a writer of the global.
export function Logged() {
  console.error("logged");
  return null;
}
