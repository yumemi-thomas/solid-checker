/// <reference types="fz-glob" />
declare function __fzProjectAct(cb: () => void): void;
interface Window {
  __fzAugAct(cb: () => void): void;
}
// An overload of a method the library declares too: the host implements the member.
interface Window {
  addEventListener(type: "fz-ready", listener: () => void): void;
}
