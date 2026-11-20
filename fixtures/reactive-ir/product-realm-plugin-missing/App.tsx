// Each site runs a member of a built-in that a package writes: `reverse` (`fz-prealm-named`) or any member (`fz-prealm-any`, which writes under a key nothing decides).
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
export function Unwritten() {
  void [1, 2].map((n) => n * 2);
  return null;
}
