import "fz-rbtoa";
import "fz-racc";
import "fz-rclear";
import "fz-rdata";

declare const flag: boolean;
declare const key: string;

// The global object is reached by many routes, and a call or a read through each is a call or a read of the global it names: `fz-rbtoa` replaces `btoa` (a data write,
// so a CALL runs its code, a plain read does not) and `fz-racc` defines an accessor for `devicePixelRatio` (a READ runs its code).

// A call of a global `fz-rbtoa` writes, by every route to the global object.
export function ComputedKey() {
  window["btoa"]("x"); // a string-literal key reads the member it spells
  return null;
}
export function ComputedKeyOfGlobalThis() {
  globalThis["btoa"]("x");
  return null;
}
export function ConstKey() {
  const k = "btoa";
  window[k]("x"); // a name that only holds a string
  return null;
}
export function SelfChain() {
  window.self.btoa("x"); // a member that gives the global object back
  return null;
}
export function WindowOfWindow() {
  window.window.btoa("x");
  return null;
}
export function DefaultView() {
  document.defaultView!.btoa("x");
  return null;
}
export function Aliased() {
  const w = window;
  w.btoa("x"); // a name the global object is bound to
  return null;
}
export function AliasedMember() {
  const enc = window.btoa;
  enc("x");
  return null;
}
export function Destructured() {
  const { btoa: enc } = window;
  enc("x");
  return null;
}
export function DestructuredShort() {
  const { btoa } = globalThis;
  btoa("x");
  return null;
}
export function DestructuredSelf() {
  const { btoa: enc } = window.self;
  enc("x");
  return null;
}
export function ArrayElement() {
  [btoa][0]("x");
  return null;
}
export function Either() {
  (flag ? window : self).btoa("x"); // an operand may be the global object
  return null;
}
export function Returned() {
  function getWindow() {
    return window;
  }
  getWindow().btoa("x");
  return null;
}

// A key the program does not spell may be any global: a call of it may run the code of any package that writes one, a read of it that of any that defines an accessor.
export function Unspelled() {
  window[key]("x");
  return null;
}
export function UnspelledRead() {
  void window[key];
  return null;
}

// The global object handed to a call: what the callee reads off it is not followed, so the call is a site wherever a package that may reach a Solid action writes a global.
const reader = (w: Window) => w.btoa("x");
export function HandedOn() {
  reader(window);
  return null;
}
export function HandedAliased() {
  const w = globalThis;
  reader(w);
  return null;
}

// A read of a global `fz-racc` defines an accessor for: the accessor runs where the global is read, by every route.
export function ReadMember() {
  void window.devicePixelRatio;
  return null;
}
export function ReadBare() {
  void devicePixelRatio;
  return null;
}
export function ReadTypeof() {
  void typeof window.devicePixelRatio;
  return null;
}
export function ReadComputed() {
  void window["devicePixelRatio"];
  return null;
}
export function ReadPattern() {
  const { devicePixelRatio: ratio } = window;
  void ratio;
  return null;
}
export function ReadAliased() {
  const w = globalThis;
  void w.devicePixelRatio;
  return null;
}

// Silent: the writer is clear (`atob`, `innerWidth`), no package writes the global (`screenX`), a data write is read (`unescape`: reading it runs nothing, calling it
// does), and a name the file declares is not the global object.
export function ClearWriter() {
  atob("eA==");
  window["atob"]("eA==");
  return null;
}
export function UnwrittenName() {
  void window.innerWidth; // an accessor the clear package defines
  void screenX;
  void window.screenX;
  return null;
}
export function DataRead() {
  void window.unescape;
  void unescape;
  return null;
}
export function HandedLocal() {
  reader({ btoa: (text: string) => text } as unknown as Window); // an object the program made
  return null;
}
export function Shadowed(window: { btoa(text: string): string; devicePixelRatio: number }) {
  window.btoa("x");
  void window.devicePixelRatio;
  return null;
}
export function ShadowedByConst() {
  const globalThis = { btoa: (text: string) => text, devicePixelRatio: 1 };
  globalThis.btoa("x");
  void globalThis.devicePixelRatio;
  return null;
}
