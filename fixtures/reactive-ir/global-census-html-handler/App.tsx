import { readThis } from "fz-gthis";
import { readWindow } from "fz-gwindow";
import { readBare } from "fz-gbare";

// `index.html`'s `onload` handler writes `fzEnv`: `readBare` stays undecided. The other two are decided.
// Each package looks up a global no pinned library declares: `this.fzHelper` (a script's top-level `this`), `window.fzPolicy`, a bare `fzEnv`. Nothing in this
// program writes any of them (no package, no project file, no script of `index.html`), so each is the host's or absent, and the call runs no action.
export function ReadThis() {
  return <p>{readThis()}</p>;
}
export function ReadWindow() {
  return <p>{readWindow()}</p>;
}
export function ReadBare() {
  return <p>{readBare()}</p>;
}
