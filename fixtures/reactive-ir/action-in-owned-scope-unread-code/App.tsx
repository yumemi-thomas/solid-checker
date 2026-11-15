import "fz-late";

// ADR 1002: code of a package that nobody read may write any global. `fz-late` loads `./late.js` by a computed name; the build reads no further, and that
// file replaces `btoa` with a function that calls a Solid action. Chrome rc.21 throws ACTION_CALLED_IN_OWNED_SCOPE at the call in a component body.

// U (package-may-call-action): the global the unread file replaced.
export function Plain() {
  btoa("x");
  return null;
}

// U: any other global of the standard library may be the one it replaced.
export function Other() {
  atob("eA==");
  return null;
}

// U: the same call through the global object.
export function Member() {
  window.btoa("x");
  return null;
}

// silent: a local of the same name is no global.
export function Shadowed() {
  const btoa = (text: string) => text;
  btoa("x");
  return null;
}
