import "fz-pub";
import "fz-over";
import { clearStep } from "fz-win-clear";
import { docViewStep, twiceStep, winDirectStep, winSelfStep } from "fz-win";

// B4: a global the build's own packages can provide (`fz-pub` writes `window.__fzAct`), reached by a name the scan did not follow.
export function WindowSelf() {
  winSelfStep(() => {}); // `window.self.__fzAct`: the global object through its own member
  return null;
}
export function WindowDirect() {
  winDirectStep(() => {}); // `window.__fzAct`: the twin the scan already read
  return null;
}
export function DocumentView() {
  docViewStep(() => {}); // `document.defaultView.__fzAct`
  return null;
}
export function GlobalThisTwice() {
  twiceStep(() => {}); // `globalThis.globalThis.__fzAct`
  return null;
}
// Silent: every member read off the global object is one the libraries declare.
export function WindowSelfClear() {
  clearStep(() => {});
  return null;
}

// B4: a call of a global some declaration file declares says nothing of who provides it.
export function TypesGlobal() {
  __fzTypesAct(() => {}); // `@types/fz-glob` holds globals and no module
  return null;
}
export function ProjectGlobal() {
  __fzProjectAct(() => {}); // `declare function` in the project's own declaration file
  return null;
}
export function WindowMember() {
  window.__fzAugAct(() => {}); // a member of `window` the project's `interface Window` adds
  return null;
}
export function GlobalThisMember() {
  globalThis.__fzProjectAct(() => {}); // a member of `globalThis` the project's `declare function` adds
  return null;
}
export function ValueGlobal() {
  [() => {}].forEach(__fzProjectAct); // handed on as a value
  return null;
}
export function AliasGlobal() {
  const run = __fzProjectAct; // a binding that only names it
  run(() => {});
  return null;
}
// Silent: the standard library declares each of these, so the host provides them.
export function HostMembers() {
  window.matchMedia("(min-width: 1px)");
  document.body.append("x");
  window.setTimeout(() => {}, 0);
  return null;
}
// A package adds an overload to a member the library declares too (`Uint8Array.from`): what runs for the overload is the package's.
export function OverloadByPackage() {
  Uint8Array.from(() => {});
  return null;
}
// Silent: a project's own overload of a member the library declares too (`window.addEventListener`): the host implements the member.
export function OverloadByProject() {
  window.addEventListener("fz-ready", () => {});
  return null;
}
// Silent: a handler and a timer callback are legal action sites.
export function TypesGlobalHandler() {
  return <button onClick={() => __fzTypesAct(() => {})}>go</button>;
}
export function TypesGlobalTimer() {
  setTimeout(() => __fzTypesAct(() => {}), 0);
  return null;
}
