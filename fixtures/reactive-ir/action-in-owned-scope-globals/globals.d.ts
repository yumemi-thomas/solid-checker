/// <reference types="fz-glob" />
declare function __fzProjectAct(cb: () => void): void;
interface Window {
  __fzAugAct(cb: () => void): void;
}
