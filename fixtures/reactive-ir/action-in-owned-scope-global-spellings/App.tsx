import "fz-sbtoa";
import "fz-sview";

// ADR 0999: a global is compared by the name its spelling means, and the package side reads `x["defaultView"]` as `x.defaultView`.
// `fz-sbtoa` replaces `btoa` (`globalThis.btoa = …`); `fz-sview` replaces `atob` through `document.documentElement.ownerDocument["defaultView"]`.
// Each replacement calls a Solid action: Chrome rc.21 throws ACTION_CALLED_IN_OWNED_SCOPE at the call in a component body.

// U (package-may-call-action): the control, the name as it is.
export function Plain() {
  btoa("x");
  return null;
}

// U: an identifier written with a Unicode escape is the name it spells.
export function EscapedBare() {
  \u0062toa("x");
  return null;
}
export function EscapedBraced() {
  \u{62}toa("x");
  return null;
}
export function EscapedMember() {
  window.\u0062toa("x");
  return null;
}
export function EscapedGlobalObject() {
  \u0077indow.btoa("x");
  return null;
}

// U: the writer is found through the computed `["defaultView"]`.
export function ViewWritten() {
  atob("x");
  return null;
}

// silent: a global no package writes.
export function Unwritten() {
  unescape("x");
  return null;
}

// silent: a local of the same name is no global.
export function Shadowed() {
  const btoa = (text: string) => text;
  btoa("x");
  return null;
}
